// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-74** — test host cho `Tcb.SetTls` (method **15** của `[object].Tcb` trong
//! `spec/abi.toml`) qua **invocation** (syscall 0 trên cap TCB).
//!
//! Vì sao test host: đây là ngữ nghĩa **core** (cap/quyền/arity/trạng thái), và `Tcb.SetTls`
//! **không** phụ thuộc hook arch (không VM) ⇒ kiểm được không cần QEMU. Bằng chứng QEMU
//! (`# tls:`, `x4`/`tp` quan sát **từ trapframe tại trap**) nằm ở
//! `kernel/vifast-kbin-riscv64/src/ktest_exec.rs` — TAP `ktest-exec`.
//!
//! Ranh giới đã kiểm: (a) số method **lấy từ spec** (`vifast_abi::TCB_SETTLS`, không chép tay);
//! (b) arity **chính xác 1** (D2-104) — thiếu/thừa đều `InvalidArgument` 5; (c) cap TCB phải có
//! `RIGHT_WRITE` ⇒ thiếu là `InsufficientRights` 3; (d) cap sai loại/method chưa cài ⇒
//! `NotSupported` 18 (D2-53 Q11, không trả `Ok` giả); (e) **không** đổi trạng thái TCB; (f) giá
//! trị đọc lại **đúng** qua `Machine::tcb_tls` (một nguồn cho arch khi dựng trapframe).
#![cfg(test)]

extern crate std;

use crate::error::{KError, ObjId, ObjType, SlotRef};
use crate::invoke;
use crate::machine::Machine;
use crate::spec::{RIGHTS_ALL, RIGHT_READ, RIGHT_WRITE};
use crate::syscall::SyscallEnv;
use crate::tcb::TcbState;
use crate::tcbdata::TcbData;

/// Hook arch tối thiểu — `Tcb.SetTls` không dùng hook nào (D2-83 chỉ VM mới cần).
struct NoEnv;

impl SyscallEnv for NoEnv {
    fn put_str(&mut self, _ptr: u64, len: u64) -> Result<u64, u64> {
        Ok(len)
    }
    fn resolve_addr(&mut self, _va: u64) -> Result<(u32, u32), u64> {
        Err(11)
    }
    fn now_ns(&mut self) -> u64 {
        0
    }
    fn features(&mut self) -> u64 {
        0
    }
    fn num_harts(&mut self) -> u64 {
        1
    }
}

/// Khoá **nối tiếp** cho các ca đọc **bộ đếm toàn cục** `invoke::stats()`.
///
/// `set_tls` là bộ đếm **dùng chung** giữa các test chạy song song trong cùng tiến trình; muốn
/// khẳng định **chính xác** ("ca này cộng đúng hai", "ca âm không cộng") thì các ca đó phải chạy
/// **tuần tự** — nếu không, kết quả phụ thuộc lịch chạy (đúng loại flake D2-108B). Trong module
/// này chỉ **ba** ca ghi bộ đếm đó nên một mutex là đủ; không dùng cho ca không đọc bộ đếm.
static TLS_COUNTER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// `MessageInfo`: `length` (bit 28..) = số MR dùng, `label` (bit 0..) = method, `caps = 0`.
fn info(label: u32, len: u8) -> u64 {
    u64::from(len) << 28 | u64::from(label)
}

fn invoke_on(
    m: &mut Machine,
    tcb: ObjId,
    call_slot: SlotRef,
    label: u32,
    len: u8,
    mrs: [u64; 4],
) -> Result<(u64, u64), u64> {
    invoke::dispatch(m, tcb, call_slot, info(label, len), &mrs, &mut NoEnv)
}

/// Cảnh một tiến trình: CNode 16 slot; slot 0 = cap CNode tự (RIGHTS_ALL), slot 1 = TCB,
/// slot 10 = Untyped (giữ để retype cap phụ), slot 11 = cap TCB **chỉ đọc** (ca quyền).
struct Rig {
    m: Machine,
    tcb: ObjId,
    c: ObjId,
    tcb_slot: SlotRef,
    ro_tcb_slot: SlotRef,
    u_slot: SlotRef,
}

fn rig() -> Rig {
    let mut m = Machine::<crate::observer::TestObserver>::new();
    let u = m.add_untyped(0x8200_0000, 1 << 16);
    let c = m.add_cnode(16);
    let self_cap = m.root_cap(c);
    m.put_cap(c, 0, self_cap).expect("self cap");
    let u_cap = m.root_cap(u);
    m.put_cap(c, 10, u_cap).expect("cap untyped");
    let tcb_cap = m
        .retype(SlotRef::new(c, 10), ObjType::Tcb, 0, 0)
        .expect("retype tcb");
    m.put_cap(c, 1, tcb_cap).expect("cap tcb");
    m.tcb_configure(SlotRef::new(c, 1), Some(SlotRef::new(c, 0)))
        .expect("cspace = c");
    // Cap TCB **chỉ đọc**: Mint bản sao quyền `RIGHT_READ` vào slot 11 (ca quyền âm).
    m.mint(SlotRef::new(c, 1), SlotRef::new(c, 11), RIGHT_READ, None)
        .expect("mint tcb ro");
    let tcb = m.cap_obj(tcb_cap).expect("obj tcb");
    Rig {
        m,
        tcb,
        c,
        tcb_slot: SlotRef::new(c, 1),
        ro_tcb_slot: SlotRef::new(c, 11),
        u_slot: SlotRef::new(c, 10),
    }
}

/// **Số method lấy từ spec, không chép tay** — `[object].Tcb` khai `SetTls = 15` và hằng core
/// bằng đúng số đó; `is_whitelisted` phải nhận cặp `(Tcb, SetTls)`.
#[test]
fn so_method_settls_sinh_tu_spec() {
    assert_eq!(invoke::TCB_SET_TLS, vifast_abi::TCB_SETTLS);
    assert_eq!(invoke::TCB_SET_TLS, 15);
    assert!(invoke::tcb_set_tls_matches_spec());
    assert!(vifast_abi::TCB_METHODS
        .iter()
        .any(|(name, id)| *name == "SetTls" && u32::from(*id) == invoke::TCB_SET_TLS));
    assert!(invoke::is_whitelisted(ObjType::Tcb, invoke::TCB_SET_TLS));
    // Method **chưa cài** của Tcb (4 = SetPriority) **không** nằm trong danh sách trắng.
    assert!(!invoke::is_whitelisted(ObjType::Tcb, 4));
}

/// `SetTls(tls)` ghi đúng giá trị: đọc lại qua `tcb_tls` **và** qua `TcbInfo.tls` (một nguồn),
/// trả `a1 = 0`, và **không** đổi trạng thái TCB.
#[test]
fn set_tls_ghi_va_doc_lai() {
    let _guard = TLS_COUNTER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    // Bộ đếm `invoke::stats()` là **toàn cục** và test chạy **song song** ⇒ khẳng định **delta**
    // của chính ca này (cùng bài học D2-108B với `capalloc_tests`), không khẳng định trị tuyệt đối.
    let base_set_tls = invoke::stats().set_tls;
    let before = r.m.tcb_state(r.tcb).expect("state");
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_SET_TLS,
            1,
            [0xdead_beef_0000_1234, 0, 0, 0]
        ),
        Ok((0, 0))
    );
    assert_eq!(
        r.m.tcb_tls(r.tcb).expect("tls"),
        0xdead_beef_0000_1234,
        "tcb_tls phải trả giá trị vừa SetTls"
    );
    assert_eq!(
        r.m.tcb_info_obj(r.tcb).expect("info").tls,
        0xdead_beef_0000_1234
    );
    assert_eq!(
        r.m.tcb_state(r.tcb).expect("state"),
        before,
        "SetTls không đổi trạng thái"
    );
    // Lần hai ghi đè (không cộng dồn): TLS là **giá trị**, không phải bộ đếm.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_SET_TLS,
            1,
            [7, 0, 0, 0]
        ),
        Ok((0, 0))
    );
    assert_eq!(r.m.tcb_tls(r.tcb).expect("tls"), 7);
    assert!(
        invoke::stats().set_tls >= base_set_tls + 2,
        "delta set_tls: {} -> {} (ca này gọi hai lần)",
        base_set_tls,
        invoke::stats().set_tls
    );
    let _ = (r.c, RIGHTS_ALL);
}

/// Arity **chính xác 1** (D2-104): `len = 0` (thiếu) và `len = 2`/`4` (thừa) đều
/// `InvalidArgument` **5** — và **không** ghi giá trị.
#[test]
fn set_tls_arity_chinh_xac() {
    let _guard = TLS_COUNTER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let base_set_tls = invoke::stats().set_tls;
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_SET_TLS,
            0,
            [9, 9, 9, 9]
        ),
        Err(KError::InvalidArgument.id() as u64)
    );
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_SET_TLS,
            2,
            [9, 9, 9, 9]
        ),
        Err(KError::InvalidArgument.id() as u64)
    );
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_SET_TLS,
            4,
            [9, 9, 9, 9]
        ),
        Err(KError::InvalidArgument.id() as u64)
    );
    assert_eq!(
        r.m.tcb_tls(r.tcb).expect("tls"),
        0,
        "arity sai không được ghi TLS"
    );
    assert_eq!(
        invoke::stats().set_tls,
        base_set_tls,
        "arity sai không được tăng bộ đếm SetTls"
    );
}

/// Cap TCB **thiếu `RIGHT_WRITE`** ⇒ `InsufficientRights` **3** (cùng luật quyền D2-99(e)),
/// và **không** ghi giá trị.
#[test]
fn set_tls_thieu_quyen_ghi() {
    let mut r = rig();
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.ro_tcb_slot,
            invoke::TCB_SET_TLS,
            1,
            [0x55, 0, 0, 0]
        ),
        Err(KError::InsufficientRights.id() as u64)
    );
    assert_eq!(r.m.tcb_tls(r.tcb).expect("tls"), 0);
}

/// Method **chưa cài** của Tcb (4 `SetPriority`) ⇒ `NotSupported` **18**; method ngoài dải
/// (16) cũng 18; cap **không phải TCB** (Untyped) với label 15 ⇒ 18 — không trả `Ok` giả.
#[test]
fn set_tls_phuong_thuc_va_cap_sai() {
    let mut r = rig();
    assert_eq!(
        invoke_on(&mut r.m, r.tcb, r.tcb_slot, 4, 1, [1, 0, 0, 0]),
        Err(KError::NotSupported.id() as u64),
        "Tcb.SetPriority (4) chưa cài ⇒ 18"
    );
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_SET_TLS + 1,
            1,
            [1, 0, 0, 0]
        ),
        Err(KError::NotSupported.id() as u64),
        "method 16 không có trong [object].Tcb ⇒ 18"
    );
    // Cap Untyped + label 15: cap sai loại ⇒ 18 (không phải 1/2) — khớp bảng whitelist hẹp.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.u_slot,
            invoke::TCB_SET_TLS,
            1,
            [1, 0, 0, 0]
        ),
        Err(KError::NotSupported.id() as u64),
        "invocation 15 trên Untyped ⇒ 18"
    );
    assert_eq!(r.m.tcb_tls(r.tcb).expect("tls"), 0);
}

/// `SetTls` ghi được khi TCB **`Ready`** (không đòi `Inactive`): giá trị có hiệu lực ở lần vào
/// U-mode kế tiếp, nên chuỗi `exec → SetTls → resume` chạy được. Đây là **khác biệt có chủ ý**
/// so với seL4 (`TCB_SetTLSBase` đòi `Inactive`) — ghi ở `docs/k6-exec.md` §3.
#[test]
fn set_tls_khi_ready_van_ghi_duoc() {
    let _guard = TLS_COUNTER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    r.m.start_tcb(r.tcb).expect("Ready");
    assert_eq!(r.m.tcb_state(r.tcb).expect("state"), TcbState::Ready);
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_SET_TLS,
            1,
            [0x9000, 0, 0, 0]
        ),
        Ok((0, 0)),
        "TCB Ready vẫn SetTls được (giá trị dùng ở lần resume kế tiếp)"
    );
    assert_eq!(r.m.tcb_tls(r.tcb).expect("tls"), 0x9000);
    // Vẫn `Ready` — `SetTls` không đổi trạng thái/scheduler.
    assert_eq!(r.m.tcb_state(r.tcb).expect("state"), TcbState::Ready);
    let _ = RIGHT_WRITE;
}

/// **KT-74** — chứng minh **phép tính** của việc tăng pin (không phải "tăng 512 là hợp lý"):
/// `TcbData` thêm **đúng một** `u64` TLS, `TcbData` là phần tử của `Machine` theo trần
/// `MAX_TCBS` ⇒ `size_of::<Machine>()` tăng **đúng** `8 × MAX_TCBS`.
///
/// Pin cũ (D2-84/85, trước KT-74) = **188872** B — hằng **lịch sử** ghi trong
/// `docs/k3b-core.md` §13.7; pin `196608` **không** đổi. Test này là chỗ reviewer kiểm phép
/// tính mà không phải tin lời khai:
/// `188872 + 8 × 64 = 189384`.
///
/// **KT-282** — `TcbData` thêm **đúng một** `Option<IpcBufferBinding>` (`ObjId` 4 B + `va` 8 B,
/// căn 8 ⇒ **24 B**) ⇒ mốc KT-74 `189384` + `24 × 64 = 1536` = **190920** B. Pin `196608`
/// **vẫn không đổi** (headroom còn 5.688 B); đây là phép tính, không phải nới ngưỡng.
///
/// **KT-309** — thêm **ba** khoản, mỗi khoản có phép tính riêng:
/// (a) `TcbData.extra_caps_dst: Option<u16>` — `TcbData` **816** B (trước 808) ⇒ `+8 × 64 = 512`;
/// (b) `Message.extra_snaps: [CapSnapshot; 4]` + `n_extra: u8` — `Message` **+128 B** (đệm căn 8
///     của `CapSnapshot`) ⇒ `+128 × MAX_QUEUED_MESSAGES(24) = 3072`;
/// (c) hai bộ đếm `u64` (`extra_caps_sent`/`extra_caps_installed`) ⇒ `+16`.
/// Tổng `190920 + 512 + 3072 + 16 = 194520` B — pin `196608` **vẫn không đổi** (headroom 2.088).
///
/// **KT-311** — `TcbData.entry: u64` (tham số `entry?` của `Tcb.Configure`) — `TcbData` **824** B
/// (trước 816) ⇒ `+8 × 64 = 512`.
/// **KT-312** — **giảm** `40 B × MAX_OBJECTS(256) = 10240` B: `Meta` **88 → 48 B** nhờ thu hẹp
/// ba trường chỉ số (`untyped`/`region`: `Option<usize>` 16 B → `Option<u8>` 2 B; `slots_meta`:
/// `Option<usize>` 16 B → `Option<u16>` 4 B) — miền giá trị **chứng minh được** (`< MAX_UNTYPEDS
/// = 16`, `≤ MAX_SLOT_WORDS = 4096`), ngữ nghĩa **không đổi**.
/// Tổng sau **cả hai**: `194520 + 512 − 10240 = 184792` B — pin `196608` **vẫn không đổi**
/// (headroom **11.816**). Phép tính là **đại số có dấu** (tăng có ghi, **giảm có ghi**),
/// không phải nới ngưỡng.
#[test]
fn pin_machine_tang_dung_8_byte_moi_tcb() {
    const PIN_CU: usize = 188_872;
    /// Mốc KT-74 (`188872 + 8 × 64`) — trước KT-282.
    const KT74_CU: usize = 189_384;
    /// **KT-282**: `Option<IpcBufferBinding>` mỗi TCB (`ObjId` 4 B + `va` 8 B, căn 8).
    const KT282_THEM_MOI_TCB: usize = 24;
    /// **KT-309**: `Option<u16>` mỗi TCB (đệm căn 8 ⇒ 8 B).
    const KT309_THEM_MOI_TCB: usize = 8;
    /// **KT-309**: `Message` lớn thêm 128 B (`extra_snaps[4]` = 96 B + `n_extra` + đệm căn 8).
    const KT309_THEM_MOI_MSG: usize = 128;
    /// **KT-309**: hai bộ đếm `u64` vô hướng.
    const KT309_THEM_SCALAR: usize = 16;
    /// **KT-311**: `TcbData.entry: u64` mỗi TCB (căn 8 ⇒ 8 B).
    const KT311_THEM_MOI_TCB: usize = 8;
    /// **KT-312**: `Meta` 88 → 48 B (thu hẹp ba trường chỉ số) ⇒ **giảm** `40 × MAX_OBJECTS`.
    const KT312_GIAM_MOI_META: usize = 40;
    /// **KT-405** — bốn khoản, mỗi khoản có phép tính riêng:
    /// (a) gộp `sched`/`published`/`mail`/`tlb_pending` thành `PerHartState` **+48 B**
    ///     (6 B đệm × 8 hart);
    /// (b) thu hẹp `HartSched.len` `usize → u16` **−64 B** (8 B × 8 hart);
    /// (c) tách `EndpointCore` (48 B) khỏi `EndpointModel` (16 B) — `EndpointData` 80 → 64 B
    ///     ⇒ **−256 B** (16 B × `MAX_ENDPOINTS` 16);
    /// (d) `QueuedMsg.next: u16` (mắt xích xâm nhập) đẩy `QueuedMsg` 848 → 856 B
    ///     ⇒ **+192 B** (8 B × `MAX_QUEUED_MESSAGES` 24).
    /// Tổng **giảm ròng 80 B**. Đây là đại số có dấu (tăng có ghi, giảm có ghi), không nới pin.
    const KT405_GIAM_RONG: usize = 80;
    let them = core::mem::size_of::<Machine>() as isize - PIN_CU as isize;
    let tcb = core::mem::size_of::<TcbData>();
    let msg = core::mem::size_of::<crate::ipc::Message>();
    let tang = (8 * crate::MAX_TCBS
        + KT282_THEM_MOI_TCB * crate::MAX_TCBS
        + KT309_THEM_MOI_TCB * crate::MAX_TCBS
        + KT309_THEM_MOI_MSG * crate::MAX_QUEUED_MESSAGES
        + KT309_THEM_SCALAR
        + KT311_THEM_MOI_TCB * crate::MAX_TCBS) as isize;
    let giam = (KT312_GIAM_MOI_META * crate::MAX_OBJECTS + KT405_GIAM_RONG) as isize;
    let tinh = tang - giam;
    std::println!(
        "# tls pin: size_of_machine={} pin_cu={} delta={} size_of_tcbdata={} size_of_message={} max_tcbs={} 8xmax={} kt282={} kt309_tcb={} kt309_msg={} kt309_scalar={} kt311_tcb={} kt312_giam={} tinh={} khop={} duoi_pin={}",
        core::mem::size_of::<Machine>(),
        PIN_CU,
        them,
        tcb,
        msg,
        crate::MAX_TCBS,
        8 * crate::MAX_TCBS,
        KT282_THEM_MOI_TCB * crate::MAX_TCBS,
        KT309_THEM_MOI_TCB * crate::MAX_TCBS,
        KT309_THEM_MOI_MSG * crate::MAX_QUEUED_MESSAGES,
        KT309_THEM_SCALAR,
        KT311_THEM_MOI_TCB * crate::MAX_TCBS,
        giam,
        tinh,
        u8::from(them == tinh),
        u8::from(core::mem::size_of::<Machine>() <= 196_608)
    );
    assert_eq!(
        them, tinh,
        "tăng pin phải đúng 8 B (TLS) + 24 B (IPC buffer) + 8 B (extra_caps_dst) × MAX_TCBS \
         + 128 B × MAX_QUEUED_MESSAGES + 16 B (hai bộ đếm) + 8 B (entry) × MAX_TCBS \
         − 40 B × MAX_OBJECTS (KT-312) − 80 B (KT-405: PerHartState +48, HartSched.len −64, \
         EndpointCore −256, QueuedMsg.next +192)"
    );
    assert_eq!(
        core::mem::size_of::<Machine>() as isize - KT74_CU as isize,
        (KT282_THEM_MOI_TCB * crate::MAX_TCBS
            + KT309_THEM_MOI_TCB * crate::MAX_TCBS
            + KT309_THEM_MOI_MSG * crate::MAX_QUEUED_MESSAGES
            + KT309_THEM_SCALAR
            + KT311_THEM_MOI_TCB * crate::MAX_TCBS) as isize
            - (KT312_GIAM_MOI_META * crate::MAX_OBJECTS + KT405_GIAM_RONG) as isize,
        "KT-282 + KT-309 + KT-311 (tăng) và KT-312 (giảm) là **toàn bộ** phần lệch so với mốc KT-74"
    );
    // **KT-312** — `Meta` phải đúng 48 B: nếu ai thêm lại `Option<usize>` vào `Meta`, phép
    // tính trên lệch ngay (không có chỗ "trôi").
    assert_eq!(
        core::mem::size_of::<crate::machine::Meta>(),
        48,
        "Meta phải 48 B (KT-312)"
    );
    assert_eq!(crate::MAX_TCBS, 64);
    assert_eq!(crate::MAX_QUEUED_MESSAGES, 24);
    assert!(
        core::mem::size_of::<Machine>() <= 196_608,
        "vẫn dưới pin 196608"
    );
    // `TcbData` có đúng một trường TLS: nếu ai thêm trường thứ hai, phép tính trên sẽ lệch.
    assert_eq!(tcb % 8, 0, "TcbData căn 8");
    assert_eq!(msg % 8, 0, "Message căn 8");
}
