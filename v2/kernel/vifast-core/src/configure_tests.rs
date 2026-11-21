// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-290** — test host cho `Tcb.Configure` (method **0** của `[object].Tcb` trong
//! `spec/abi.toml`) qua **invocation** (syscall 0 trên cap TCB), phần **bind IPC buffer**.
//!
//! **KT-311** — mở rộng: kênh **đầy đủ** (`length = 3`, `MR2` = mask) cho `cspace?`/`entry?`/
//! `priority?`; giá trị đọc từ **vùng riêng** của IPC buffer (byte 608..640, `[TK]` KT-311) qua
//! hook `SyscallEnv::ipc_buffer_configure` (D2-83: core không đọc bộ nhớ user). `vspace?` **chưa
//! cài** ⇒ bật bit ⇒ `NotSupported` 18.
//!
//! Vì sao test host: đây là ngữ nghĩa **core** (cap/quyền/arity/loại Frame/VA/CSpace/entry/
//! priority) và phần này **không** phụ thuộc hook arch (không VM) ⇒ kiểm được không cần QEMU.
//! Bằng chứng QEMU (`# ipc: configure2 …`, TAP `ktest-ipc` #6) nằm ở
//! `kernel/vifast-kbin-riscv64/src/ktest_ipc.rs` — chương trình U-mode thật gọi `Call`.
//!
//! Ranh giới đã kiểm: (a) số method **lấy từ spec** (`vifast_abi::TCB_CONFIGURE`, không chép
//! tay); (b) arity **chính xác 2 hoặc 3** (D2-104) — còn lại `InvalidArgument` 5; (c) cap TCB
//! phải có `RIGHT_WRITE` ⇒ thiếu là `InsufficientRights` 3; (d) cap `Frame` sai loại/chết ⇒
//! `WrongObjectType` 2 / `InvalidCapability` 1; (e) `va` không căn 4 KiB hoặc tràn `u64` ⇒ 5;
//! (f) **CSpace giữ nguyên** khi không có `cspace?` (khác `tcb_configure(tcb, None)` — API đó xoá
//! CSpace); (g) `exit_code` bị xoá (D2-25); (h) **không** đổi trạng thái TCB; (i) cap không phải
//! TCB / method chưa cài ⇒ `NotSupported` 18 (D2-53 Q11, không trả `Ok` giả); (j) **KT-311**
//! `cspace?` phải là `CNode` sống ⇒ không: 2/1; `entry?` căn 2 B (biên lệnh RISC-V có `C`) ⇒ không: `Alignment` 6;
//! `priority?` vừa `u8` + ≤ `mcp` ⇒ không: 5; mask bit lạ ⇒ 5; `vspace?` ⇒ 18; **nguyên tử**.
#![cfg(test)]

extern crate std;

use crate::error::{KError, ObjId, ObjType, SlotRef};
use crate::invoke;
use crate::machine::Machine;
use crate::spec::RIGHT_READ;
use crate::syscall::SyscallEnv;

/// Hook arch tối thiểu — `Tcb.Configure` (phần `ipc_buffer?`) không dùng hook nào (D2-83 chỉ VM
/// mới cần).
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

/// **KT-311** — hook arch giả trả **header IPC buffer** với 4 word tham số mở rộng (`cspace`,
/// `vspace`, `entry`, `priority`) — đúng dữ liệu arch đọc từ vùng 608..640 của IPC buffer.
/// `None` = trang chưa map / magic chưa ghi (ca âm fail-closed).
struct CfgEnv(Option<[u64; 4]>);

impl SyscallEnv for CfgEnv {
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
    fn ipc_header(&mut self, _tcb: ObjId) -> Option<crate::ipc::IpcHeader> {
        self.0.map(|words| crate::ipc::IpcHeader {
            info: 0,
            recv_window: [0; 4],
            send_caps: [0; 4],
            extra: crate::ipc::IpcExtraCaps::EMPTY,
            configure: crate::ipc::IpcConfigureParams::from_words(&words),
        })
    }
}

/// Khoá **nối tiếp** cho các ca đọc **bộ đếm toàn cục** `invoke::stats()` (cùng bài học D2-108B
/// với `tls_tests`): `configure` là bộ đếm dùng chung giữa các test chạy song song.
static CONFIGURE_COUNTER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

/// **KT-311** — như [`invoke_on`] nhưng với hook arch trả 4 word tham số mở rộng.
fn invoke_on_cfg(
    m: &mut Machine,
    tcb: ObjId,
    call_slot: SlotRef,
    len: u8,
    mrs: [u64; 4],
    words: Option<[u64; 4]>,
) -> Result<(u64, u64), u64> {
    invoke::dispatch(
        m,
        tcb,
        call_slot,
        info(invoke::TCB_CONFIGURE, len),
        &mrs,
        &mut CfgEnv(words),
    )
}

/// Cảnh một tiến trình: CNode 16 slot; slot 0 = cap CNode tự (RIGHTS_ALL), slot 1 = TCB,
/// slot 2 = Frame 4 KiB (IPC buffer), slot 10 = Untyped, slot 11 = cap TCB **chỉ đọc**,
/// slot 12 = VSpace (cap **không phải** TCB cho ca âm), slot 13 = **CNode thứ hai** (KT-311 —
/// `cspace?` trỏ sang CSpace khác), slot 14 = Endpoint (cap **không phải** CNode cho ca âm
/// `cspace?`).
struct Rig {
    m: Machine,
    tcb: ObjId,
    c: ObjId,
    c2: ObjId,
    tcb_slot: SlotRef,
    ro_tcb_slot: SlotRef,
    frame_slot: SlotRef,
    ep_slot: SlotRef,
    cnode2_slot: SlotRef,
}

fn rig() -> Rig {
    let mut m = Machine::new();
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
    // Frame 4 KiB — object **đại diện** (D2-83: core không sở hữu PTE/PA; test host không cần
    // object layer vì `Tcb.Configure` chỉ kiểm **loại** cap + giữ danh tính).
    let frame_obj = m
        .add_foreign_object(ObjType::Frame, 0x2900_0000)
        .expect("frame đại diện");
    let frame_cap = m.root_cap(frame_obj);
    m.put_cap(c, 2, frame_cap).expect("cap frame");
    // Cap TCB **chỉ đọc**: Mint bản sao quyền `RIGHT_READ` vào slot 11 (ca quyền âm).
    m.mint(SlotRef::new(c, 1), SlotRef::new(c, 11), RIGHT_READ, None)
        .expect("mint tcb ro");
    // Cap **không phải TCB** (Endpoint) cho ca âm "cap sai loại" — `(Endpoint, 0)` không nằm
    // trong danh sách trắng ⇒ 18 (khác `(Frame, 0)` = `Frame.Map`).
    let ep_cap = m
        .retype(SlotRef::new(c, 10), ObjType::Endpoint, 0, 0)
        .expect("retype endpoint");
    m.put_cap(c, 12, ep_cap).expect("cap endpoint");
    // **KT-311** — CNode **thứ hai** (CSpace khác) cho `cspace?` đường xanh; cap Endpoint ở
    // slot 14 là ca âm "cspace không phải CNode".
    let c2 = m.add_cnode(4);
    let c2_cap = m.root_cap(c2);
    m.put_cap(c, 13, c2_cap).expect("cap cnode 2");
    let ep2_cap = m
        .retype(SlotRef::new(c, 10), ObjType::Endpoint, 0, 0)
        .expect("retype endpoint 2");
    m.put_cap(c, 14, ep2_cap).expect("cap endpoint 2");
    // **KT-311** — priority/mcp ban đầu (3/10) để `priority?` có dải thật để kiểm: 4..10 hợp lệ,
    // 11 vượt `mcp` ⇒ `InvalidArgument`.
    m.tcb_set_priority(SlotRef::new(c, 1), 3, 10)
        .expect("priority/mcp");
    let tcb = m.cap_obj(tcb_cap).expect("obj tcb");
    Rig {
        m,
        tcb,
        c,
        c2,
        tcb_slot: SlotRef::new(c, 1),
        ro_tcb_slot: SlotRef::new(c, 11),
        frame_slot: SlotRef::new(c, 2),
        ep_slot: SlotRef::new(c, 12),
        cnode2_slot: SlotRef::new(c, 13),
    }
}

/// **Số method lấy từ spec, không chép tay** — `[object].Tcb` khai `Configure = 0` và hằng core
/// bằng đúng số đó; `is_whitelisted` phải nhận cặp `(Tcb, Configure)`.
#[test]
fn so_method_configure_sinh_tu_spec() {
    assert_eq!(invoke::TCB_CONFIGURE, vifast_abi::TCB_CONFIGURE);
    assert_eq!(invoke::TCB_CONFIGURE, 0);
    assert!(vifast_abi::TCB_METHODS
        .iter()
        .any(|(name, id)| *name == "Configure" && u32::from(*id) == invoke::TCB_CONFIGURE));
    assert!(invoke::is_whitelisted(ObjType::Tcb, invoke::TCB_CONFIGURE));
    // Method **chưa cài** của Tcb (1 = SetFaultEp) **không** nằm trong danh sách trắng.
    assert!(!invoke::is_whitelisted(ObjType::Tcb, 1));
}

/// `Configure(ipc_buffer = (Frame, VA))` bind đúng: đọc lại qua `tcb_ipc_buffer_binding`, trả
/// `a1 = 0`, **CSpace giữ nguyên**, `exit_code` bị xoá, **không** đổi trạng thái TCB.
#[test]
fn configure_bind_buffer_va() {
    let _guard = CONFIGURE_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let base = invoke::stats().configure;
    let before = r.m.tcb_state(r.tcb).expect("state");
    // `exit_code` phải bị xoá bởi `Configure` (D2-25) — đặt một giá trị trước.
    r.m.tcb_data_mut(r.tcb).expect("tcb").exit_code = Some(7);
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_CONFIGURE,
            2,
            [r.frame_slot.slot as u64, 0x4002_0000, 0, 0]
        ),
        Ok((0, 0))
    );
    let binding =
        r.m.tcb_ipc_buffer_binding(r.tcb)
            .expect("binding")
            .expect("đã bind");
    assert_eq!(binding.va, 0x4002_0000);
    assert_eq!(
        r.m.tcb_info_obj(r.tcb).expect("info").cspace,
        Some(r.c),
        "invocation Configure phải GIỮ CSpace (khác tcb_configure(tcb, None))"
    );
    assert_eq!(
        r.m.tcb_info_obj(r.tcb).expect("info").exit_code,
        None,
        "Configure xoá exit_code (D2-25)"
    );
    assert_eq!(
        r.m.tcb_state(r.tcb).expect("state"),
        before,
        "Configure không đổi trạng thái"
    );
    assert!(
        invoke::stats().configure > base,
        "delta configure: {} -> {}",
        base,
        invoke::stats().configure
    );
    // Bind lại VA khác (cùng cap Frame) — ghi đè, không cộng dồn.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_CONFIGURE,
            2,
            [r.frame_slot.slot as u64, 0x4002_1000, 0, 0]
        ),
        Ok((0, 0))
    );
    assert_eq!(
        r.m.tcb_ipc_buffer_binding(r.tcb)
            .expect("binding")
            .expect("đã bind")
            .va,
        0x4002_1000
    );
}

/// Arity **chính xác 2 hoặc 3** (D2-104 + KT-311): `len = 0/1/4` đều `InvalidArgument` **5** —
/// và **không** bind buffer (nguyên tử). `len = 3` với mask = 0 tương đương `len = 2` (bind
/// buffer, không tham số mở rộng).
#[test]
fn configure_arity_chinh_xac() {
    let _guard = CONFIGURE_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let base = invoke::stats().configure;
    for len in [0u8, 1, 4] {
        assert_eq!(
            invoke_on(
                &mut r.m,
                r.tcb,
                r.tcb_slot,
                invoke::TCB_CONFIGURE,
                len,
                [r.frame_slot.slot as u64, 0x4002_0000, 0, 0]
            ),
            Err(KError::InvalidArgument.id() as u64),
            "arity {len} phải là InvalidArgument 5"
        );
    }
    assert_eq!(
        r.m.tcb_ipc_buffer_binding(r.tcb).expect("binding"),
        None,
        "arity sai không được bind buffer"
    );
    assert_eq!(
        invoke::stats().configure,
        base,
        "arity sai không được tăng bộ đếm Configure"
    );
    // **KT-311** — `len = 3` + mask = 0: đường KT-290 (chỉ bind buffer).
    assert_eq!(
        invoke_on_cfg(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            3,
            [r.frame_slot.slot as u64, 0x4002_0000, 0, 0],
            None
        ),
        Ok((0, 0)),
        "len = 3 + mask = 0 phải bind buffer như len = 2"
    );
    assert_eq!(
        r.m.tcb_ipc_buffer_binding(r.tcb)
            .expect("binding")
            .expect("đã bind")
            .va,
        0x4002_0000
    );
    assert_eq!(
        invoke::stats().configure,
        base + 1,
        "len = 3 + mask = 0 phải tăng bộ đếm Configure đúng một lần"
    );
}

/// Cap TCB **thiếu `RIGHT_WRITE`** ⇒ `InsufficientRights` **3** (cùng luật quyền D2-99(e)),
/// và **không** bind buffer.
#[test]
fn configure_thieu_quyen_ghi() {
    let mut r = rig();
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.ro_tcb_slot,
            invoke::TCB_CONFIGURE,
            2,
            [r.frame_slot.slot as u64, 0x4002_0000, 0, 0]
        ),
        Err(KError::InsufficientRights.id() as u64)
    );
    assert_eq!(r.m.tcb_ipc_buffer_binding(r.tcb).expect("binding"), None);
}

/// Cap `ipc_buffer` **sai loại** (CNode/Endpoint) ⇒ `WrongObjectType` **2**; ô **rỗng** ⇒
/// `SlotEmpty` **8**; CPTR ngoài bảng slot ⇒ `InvalidCapability` **1**; `va` không căn 4 KiB
/// hoặc tràn `u64` ⇒ **5**.
#[test]
fn configure_cap_va_sai() {
    let mut r = rig();
    // MR0 = cap CNode tự (slot 0) — không phải Frame.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_CONFIGURE,
            2,
            [0, 0x4002_0000, 0, 0]
        ),
        Err(KError::WrongObjectType.id() as u64)
    );
    // MR0 = cap Endpoint (slot 12) — không phải Frame.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_CONFIGURE,
            2,
            [r.ep_slot.slot as u64, 0x4002_0000, 0, 0]
        ),
        Err(KError::WrongObjectType.id() as u64)
    );
    // MR0 = ô rỗng (slot 5) ⇒ `SlotEmpty` 8 (cùng ngữ nghĩa `Frame.Map` với cap phụ rỗng).
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_CONFIGURE,
            2,
            [5, 0x4002_0000, 0, 0]
        ),
        Err(KError::SlotEmpty.id() as u64)
    );
    // MR0 = CPTR ngoài bảng slot (16).
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_CONFIGURE,
            2,
            [16, 0x4002_0000, 0, 0]
        ),
        Err(KError::InvalidCapability.id() as u64)
    );
    // VA không căn 4 KiB.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_CONFIGURE,
            2,
            [r.frame_slot.slot as u64, 0x4002_0001, 0, 0]
        ),
        Err(KError::InvalidArgument.id() as u64)
    );
    // VA căn nhưng `va + 4096` tràn `u64`.
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_CONFIGURE,
            2,
            [r.frame_slot.slot as u64, u64::MAX - 4095, 0, 0]
        ),
        Err(KError::InvalidArgument.id() as u64)
    );
    assert_eq!(
        r.m.tcb_ipc_buffer_binding(r.tcb).expect("binding"),
        None,
        "mọi ca sai không được bind buffer"
    );
}

/// Cap **không phải TCB** (Endpoint) + `label = Configure` ⇒ `NotSupported` **18**; method **chưa
/// cài** của Tcb (1 `SetFaultEp`) ⇒ 18; method ngoài dải (16) ⇒ 18 — không trả `Ok` giả.
#[test]
fn configure_cap_va_method_sai() {
    let mut r = rig();
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.ep_slot,
            invoke::TCB_CONFIGURE,
            2,
            [r.frame_slot.slot as u64, 0x4002_0000, 0, 0]
        ),
        Err(KError::NotSupported.id() as u64),
        "cap Endpoint + label 0 ⇒ 18 (không phải 2)"
    );
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            1,
            2,
            [r.frame_slot.slot as u64, 0x4002_0000, 0, 0]
        ),
        Err(KError::NotSupported.id() as u64),
        "Tcb.SetFaultEp (1) chưa cài ⇒ 18"
    );
    assert_eq!(
        invoke_on(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            invoke::TCB_CONFIGURE + 16,
            2,
            [r.frame_slot.slot as u64, 0x4002_0000, 0, 0]
        ),
        Err(KError::NotSupported.id() as u64),
        "method 16 không có trong [object].Tcb ⇒ 18"
    );
    assert_eq!(r.m.tcb_ipc_buffer_binding(r.tcb).expect("binding"), None);
}

// =============================================================================================
// KT-311 — kênh đầy đủ: `cspace?` / `entry?` / `priority?` (mask ở MR2, giá trị ở IPC buffer)
// =============================================================================================

/// **KT-311** — mask = `cspace|entry|priority`: cả ba tham số được áp **thật** (đọc lại từ
/// `TcbData`), `ipc_buffer?` vẫn bind, `exit_code` bị xoá (D2-25), trạng thái TCB **không** đổi.
#[test]
fn configure_kenh_day_du_ba_tham_so() {
    let _guard = CONFIGURE_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let base = invoke::stats().configure;
    let before = r.m.tcb_state(r.tcb).expect("state");
    r.m.tcb_data_mut(r.tcb).expect("tcb").exit_code = Some(9);
    let mask = crate::ipc::CONFIGURE_MASK_CSPACE
        | crate::ipc::CONFIGURE_MASK_ENTRY
        | crate::ipc::CONFIGURE_MASK_PRIORITY;
    assert_eq!(
        invoke_on_cfg(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            3,
            [r.frame_slot.slot as u64, 0x4002_0000, mask, 0],
            Some([r.cnode2_slot.slot as u64, 0, 0x4000_0000, 4])
        ),
        Ok((0, 0))
    );
    assert_eq!(
        r.m.tcb_info_obj(r.tcb).expect("info").cspace,
        Some(r.c2),
        "cspace? phải gán CNode của CPTR (không phải giữ nguyên)"
    );
    assert_eq!(
        r.m.tcb_entry(r.tcb).expect("entry"),
        0x4000_0000,
        "entry? phải ghi đúng VA"
    );
    assert_eq!(
        r.m.tcb_info_obj(r.tcb).expect("info").priority,
        4,
        "priority? phải ghi đúng mức"
    );
    assert_eq!(
        r.m.tcb_info_obj(r.tcb)
            .expect("info")
            .max_controlled_priority,
        10,
        "priority? KHÔNG đổi mcp (đó là Tcb.SetMcp)"
    );
    assert_eq!(
        r.m.tcb_ipc_buffer_binding(r.tcb)
            .expect("binding")
            .expect("đã bind")
            .va,
        0x4002_0000,
        "ipc_buffer? vẫn bind trong kênh đầy đủ"
    );
    assert_eq!(
        r.m.tcb_info_obj(r.tcb).expect("info").exit_code,
        None,
        "Configure xoá exit_code (D2-25)"
    );
    assert_eq!(
        r.m.tcb_state(r.tcb).expect("state"),
        before,
        "Configure không đổi trạng thái"
    );
    assert_eq!(
        invoke::stats().configure,
        base + 1,
        "một invocation thành công = một lần đếm"
    );
}

/// **KT-311** — `entry?` **không căn 4 B** ⇒ `Alignment` **6**; `entry` cũ **không** đổi
/// (nguyên tử).
#[test]
fn configure_entry_khong_can_le() {
    let _guard = CONFIGURE_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mask = crate::ipc::CONFIGURE_MASK_ENTRY;
    assert_eq!(
        invoke_on_cfg(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            3,
            [r.frame_slot.slot as u64, 0x4002_0000, mask, 0],
            Some([0, 0, 0x4000_0001, 0])
        ),
        Err(KError::Alignment.id() as u64),
        "entry 0x4000_0001 (lẻ) không căn 2 B ⇒ Alignment 6"
    );
    assert_eq!(r.m.tcb_entry(r.tcb).expect("entry"), 0, "entry không đổi");
    assert_eq!(
        r.m.tcb_ipc_buffer_binding(r.tcb).expect("binding"),
        None,
        "ca sai không được bind buffer (nguyên tử)"
    );
    // Căn 2 B (biên lệnh RISC-V có `C`) là hợp lệ — kể cả địa chỉ **không** căn 4 B như
    // `e_entry` thật của ảnh test (`0x4000_0112`).
    assert_eq!(
        invoke_on_cfg(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            3,
            [r.frame_slot.slot as u64, 0x4002_0000, mask, 0],
            Some([0, 0, 0x4000_0112, 0])
        ),
        Ok((0, 0))
    );
    assert_eq!(r.m.tcb_entry(r.tcb).expect("entry"), 0x4000_0112);
}

/// **KT-311** — `priority?` **ngoài dải** ⇒ `InvalidArgument` **5**: (a) không vừa `u8`
/// (`256`); (b) vượt `max_controlled_priority` (luật MCS của `tcb_set_priority`). Priority cũ
/// **không** đổi.
#[test]
fn configure_priority_ngoai_dai() {
    let _guard = CONFIGURE_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mask = crate::ipc::CONFIGURE_MASK_PRIORITY;
    let before = r.m.tcb_info_obj(r.tcb).expect("info").priority;
    for bad in [256u64, 11] {
        assert_eq!(
            invoke_on_cfg(
                &mut r.m,
                r.tcb,
                r.tcb_slot,
                3,
                [r.frame_slot.slot as u64, 0x4002_0000, mask, 0],
                Some([0, 0, 0, bad])
            ),
            Err(KError::InvalidArgument.id() as u64),
            "priority {bad} ngoài dải ⇒ InvalidArgument 5"
        );
    }
    assert_eq!(
        r.m.tcb_info_obj(r.tcb).expect("info").priority,
        before,
        "priority không đổi sau ca sai"
    );
    // Biên trên hợp lệ: đúng `mcp` = 10.
    assert_eq!(
        invoke_on_cfg(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            3,
            [r.frame_slot.slot as u64, 0x4002_0000, mask, 0],
            Some([0, 0, 0, 10])
        ),
        Ok((0, 0))
    );
    assert_eq!(r.m.tcb_info_obj(r.tcb).expect("info").priority, 10);
}

/// **KT-311** — `cspace?` **không phải `CNode`** ⇒ `WrongObjectType` **2**; ô rỗng ⇒ `SlotEmpty`
/// **8**; CPTR ngoài bảng ⇒ `InvalidCapability` **1**. CSpace cũ **không** đổi.
#[test]
fn configure_cspace_khong_phai_cnode() {
    let _guard = CONFIGURE_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mask = crate::ipc::CONFIGURE_MASK_CSPACE;
    for (cptr, want) in [
        (r.frame_slot.slot as u64, KError::WrongObjectType),
        (r.ep_slot.slot as u64, KError::WrongObjectType),
        (5, KError::SlotEmpty),
        (16, KError::InvalidCapability),
    ] {
        assert_eq!(
            invoke_on_cfg(
                &mut r.m,
                r.tcb,
                r.tcb_slot,
                3,
                [r.frame_slot.slot as u64, 0x4002_0000, mask, 0],
                Some([cptr, 0, 0, 0])
            ),
            Err(want.id() as u64),
            "cspace CPTR {cptr} phải trả {}",
            want.name()
        );
    }
    assert_eq!(
        r.m.tcb_info_obj(r.tcb).expect("info").cspace,
        Some(r.c),
        "cspace không đổi sau các ca sai"
    );
}

/// **KT-311** — `vspace?` **chưa cài** ⇒ `NotSupported` **18** (fail-closed, không im lặng);
/// mask có bit lạ (≥ bit 4) ⇒ `InvalidArgument` **5**; arch không đọc được vùng (trang chưa map)
/// ⇒ `InvalidArgument` **5**.
#[test]
fn configure_vspace_va_mask_la() {
    let _guard = CONFIGURE_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    assert_eq!(
        invoke_on_cfg(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            3,
            [
                r.frame_slot.slot as u64,
                0x4002_0000,
                crate::ipc::CONFIGURE_MASK_VSPACE,
                0
            ],
            Some([0, 0, 0, 0])
        ),
        Err(KError::NotSupported.id() as u64),
        "vspace? chưa cài ⇒ 18"
    );
    assert_eq!(
        invoke_on_cfg(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            3,
            [r.frame_slot.slot as u64, 0x4002_0000, 1 << 4, 0],
            Some([0, 0, 0, 0])
        ),
        Err(KError::InvalidArgument.id() as u64),
        "mask bit lạ ⇒ 5"
    );
    assert_eq!(
        invoke_on_cfg(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            3,
            [
                r.frame_slot.slot as u64,
                0x4002_0000,
                crate::ipc::CONFIGURE_MASK_ENTRY,
                0
            ],
            None
        ),
        Err(KError::InvalidArgument.id() as u64),
        "arch không đọc được vùng 608..640 ⇒ 5 (fail-closed)"
    );
    assert_eq!(r.m.tcb_entry(r.tcb).expect("entry"), 0);
    assert_eq!(r.m.tcb_ipc_buffer_binding(r.tcb).expect("binding"), None);
}

/// **KT-311** — **nguyên tử**: `entry` hợp lệ + `priority` sai ⇒ **không** trường nào đổi
/// (`entry` vẫn 0, `ipc_buffer` vẫn chưa bind); `cspace` sai + `entry` hợp lệ ⇒ `entry` vẫn 0.
#[test]
fn configure_nguyen_tu() {
    let _guard = CONFIGURE_COUNTER_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut r = rig();
    let mask = crate::ipc::CONFIGURE_MASK_ENTRY | crate::ipc::CONFIGURE_MASK_PRIORITY;
    assert_eq!(
        invoke_on_cfg(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            3,
            [r.frame_slot.slot as u64, 0x4002_0000, mask, 0],
            Some([0, 0, 0x4000_0000, 11])
        ),
        Err(KError::InvalidArgument.id() as u64)
    );
    assert_eq!(
        r.m.tcb_entry(r.tcb).expect("entry"),
        0,
        "entry hợp lệ KHÔNG được ghi khi priority sai (nguyên tử)"
    );
    assert_eq!(
        r.m.tcb_ipc_buffer_binding(r.tcb).expect("binding"),
        None,
        "ipc_buffer KHÔNG được bind khi tham số khác sai (nguyên tử)"
    );
    let mask2 = crate::ipc::CONFIGURE_MASK_CSPACE | crate::ipc::CONFIGURE_MASK_ENTRY;
    assert_eq!(
        invoke_on_cfg(
            &mut r.m,
            r.tcb,
            r.tcb_slot,
            3,
            [r.frame_slot.slot as u64, 0x4002_0000, mask2, 0],
            Some([r.frame_slot.slot as u64, 0, 0x4000_0000, 0])
        ),
        Err(KError::WrongObjectType.id() as u64)
    );
    assert_eq!(
        r.m.tcb_entry(r.tcb).expect("entry"),
        0,
        "entry hợp lệ KHÔNG được ghi khi cspace sai (nguyên tử)"
    );
}
