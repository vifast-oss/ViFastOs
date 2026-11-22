// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-338b** — ảnh kiểm aarch64: `vifast-core::Machine` + IPC từ **EL0** qua `svc #0`.
//!
//! # Vì sao tệp này tồn tại (và vì sao nó **không** nằm trong ảnh sản phẩm)
//!
//! Ảnh sản phẩm aarch64 của `KT-338c` chỉ boot tới marker (chưa MMU/U-mode) — nên phần "core
//! chạy thật + IPC EL0" được kiểm trên ảnh **ktest** (`--features ktest`), nơi đã có bảng trang
//! thật, GICv3 4 hart và đường `eret` xuống EL0. `Machine` (184.792 B) nằm trong `.bss` ảnh
//! kiểm; ảnh sản phẩm **không** link nó (trần `.data+.bss` sản phẩm 65.536 B — D2-236/D2-243).
//! `KT-339` (shell sản phẩm) là task quyết định ngân sách để mang core vào ảnh sản phẩm.
//!
//! # Bốn ca kiểm của task (TAP 36..41) — đo ở **kernel**, không tin lời chương trình
//!
//! | TAP | Ca | Bằng chứng kernel |
//! |---|---|---|
//! | 36 | core link | `size_of::<Machine>()` ≤ pin 196.608 + static **trong ảnh** + `check_caps`/`check_bytes`/`check_all_bytes` xanh |
//! | 37 | (a) `Tcb.Configure` đổi **ưu tiên** | `priority` trong `Machine` = 9 sau invocation từ EL0 (trước = 4) |
//! | 38 | (b) gửi/nhận **1 message** qua Endpoint, **MR4..MR7** từ IPC buffer | `tcb_ipc_area(server)` có `length = 8`, MR0..3 = hằng của client, MR4..7 = hằng **chỉ có** trong buffer |
//! | 39 | (c) quyền + `VspaceOutcome` | Configure trên cap **thiếu `RIGHT_WRITE`** ⇒ **3**; tham số `vspace?` ⇒ arch **`Failed(Busy)` 14** + `TcbData.vspace` **rollback** (0) — **không** panic |
//! | 40 | (d) `Tcb.Suspend(Defer)` | điểm an toàn ⇒ `dispatch` trả `Switched`, TCB `Inactive`; `Tcb.Resume` trả `entry` ⇒ EL0 chạy tiếp **đúng** điểm vào đó |
//! | 41 | khoá `IrqMcsLock` | `DAIF.I` **tắt** trong vùng găng, **khôi phục** sau khi nhả (`acquired > 0`, `overlap = 0`, `in_cs = 0`) |
//!
//! # Vì sao **kernel** là nơi khẳng định, không phải chương trình EL0
//!
//! Chương trình EL0 chỉ *gọi* syscall; mọi kết luận lấy từ trạng thái `Machine` (qua
//! `core_bridge::with_machine`) — nếu chương trình in "ok" mà core không đổi gì thì TAP vẫn đỏ.
//! Cùng nguyên tắc `KT-319`/`KT-320` của riscv64 (D2-102: in từ dữ liệu, không chép chuỗi).

use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use vifast_core::invoke::{self, SUSPEND_MODE_DEFER};
use vifast_core::ipc::{
    MessageInfo, CONFIGURE_MASK_ENTRY, CONFIGURE_MASK_PRIORITY, CONFIGURE_MASK_VSPACE,
    IPC_BUFFER_CONFIGURE_OFFSET, IPC_BUFFER_EXTRA_CAPS_OFFSET, IPC_BUFFER_MR_OFFSET,
    IPC_HEADER_MAGIC, WORDS_IN_REGISTERS,
};
use vifast_core::spec::{RIGHTS_ALL, RIGHT_READ};
use vifast_core::syscall::{SyscallEnv, SyscallOutcome};
use vifast_core::{KError, NonReadyTarget, ObjId, ObjType, SlotRef, TcbState, TCB_STATE_COUNT};
use vifast_plat_aarch64::bootmem::SharedBootMemory;
use vifast_plat_aarch64::format::{MemAttr, Perms};
use vifast_plat_aarch64::split_mmu::KERNEL_HIGH_BASE;
use vifast_plat_aarch64::vspace::{Frame, VSpace};
use vifast_plat_aarch64::{asm, bootmem, core_bridge, debug_halt};

use crate::{tap, Console, TAP_TESTS};
use vifast_plat_aarch64::umode::EL0_STACK_TOP;

/// VA trang **IPC buffer** của pha core — ngoài trang mã (0x7000_0000) và trang stack
/// (0x7000_1000); cùng quy ước "ba trang user, không trang nào chồng trang nào".
pub const IPC_BUF_VA: u64 = 0x7000_2000;

/// Số ô của CSpace gốc của rig.
const ROOT_SLOTS: usize = 16;
/// Ô cap `Untyped` (nguồn Retype).
pub const SLOT_UNTYPED: usize = 0;
/// Ô cap `Tcb` của tác vụ EL0 (có `RIGHT_WRITE`).
pub const SLOT_TCB: usize = 1;
/// Ô cap `Tcb` của tác vụ **server** (kernel giữ; `Recv` trên Endpoint).
pub const SLOT_SERVER: usize = 2;
/// Ô cap `Endpoint` — kênh IPC của ca (b).
pub const SLOT_EP: usize = 3;
/// Ô cap `Frame` 4 KiB làm IPC buffer (`Tcb.Configure` tham số `ipc_buffer?`).
pub const SLOT_FRAME: usize = 4;
/// Ô cap `Tcb` **copy thiếu `RIGHT_WRITE`** — ca (c) kiểm quyền.
pub const SLOT_TCB_RO: usize = 5;
/// Ô cap **đại diện** `VSpace` (ngoại lai, D2-83) — ca (c) kiểm `VspaceOutcome`.
pub const SLOT_VSPACE: usize = 6;
/// Ô cap `CNode` của chính CSpace gốc (`Tcb.Configure` tham số `cspace?` không dùng, nhưng cần
/// một cap CNode để `tcb_configure` cấu hình CSpace cho TCB).
pub const SLOT_CSPACE: usize = 7;
/// Ô cap `Tcb` của **tác vụ pha core** — tác vụ mà chương trình EL0 thứ hai gọi invocation trên
/// chính nó. Tách khỏi `SLOT_TCB` vì TCB của lượt 1 đã `Exit` (Inactive) — dùng lại sẽ phải
/// `Configure` để xoá `exit_code`, làm nhiễu phép đo "ưu tiên đổi từ 4 → 9".
pub const SLOT_CORE: usize = 8;

/// Ưu tiên **trước** khi `Tcb.Configure` đổi (kernel đặt lúc dựng rig).
const PRIORITY_OLD: u8 = 4;
/// Ưu tiên **sau** khi `Tcb.Configure` đổi — chương trình EL0 yêu cầu trị này qua IPC buffer.
const PRIORITY_SET: u8 = 9;

/// MR0..MR3 của message ca (b) — chỉ đi trong **thanh ghi**.
pub const MSG_MR0: u64 = 0x338b_0000;
pub const MSG_MR1: u64 = 0x338b_0001;
pub const MSG_MR2: u64 = 0x338b_0002;
pub const MSG_MR3: u64 = 0x338b_0003;
/// MR4..MR7 — chỉ có trong **IPC buffer** (offset [`IPC_BUFFER_MR_OFFSET`] = 80 B).
const MSG_MR4: u64 = 0x338b_0004;
const MSG_MR5: u64 = 0x338b_0005;
const MSG_MR6: u64 = 0x338b_0006;
const MSG_MR7: u64 = 0x338b_0007;

/// Label của message ca (b).
pub const MSG_LABEL: u32 = 0x338;

/// `MessageInfo` của `Call` với `Tcb.Configure` **kênh đầy đủ** (`length = 3`) — dựng bằng
/// `MessageInfo::new(..).pack()` **tại thời điểm biên dịch** (KT-338b: `pack`/`new` thành
/// `const fn`), nên stub EL0 và core **không thể** lệch bit layout (D2-11).
pub const INFO_CFG3: u64 = msg_info(vifast_core::invoke::TCB_CONFIGURE, 3);
/// `MessageInfo` của `Tcb.Configure` **kênh hẹp** (`length = 2` — chỉ `ipc_buffer?`).
pub const INFO_CFG2: u64 = msg_info(vifast_core::invoke::TCB_CONFIGURE, 2);
/// `MessageInfo` của message ca (b) — label riêng, `length = 8`.
pub const INFO_SEND: u64 = msg_info(MSG_LABEL, 8);
/// `MessageInfo` của `Tcb.Suspend` (`length = 1` — mode).
pub const INFO_SUSPEND: u64 = msg_info(vifast_core::invoke::TCB_SUSPEND, 1);
/// Mask `priority?` của `Tcb.Configure` (bit 3) — lấy từ **hằng core**, không chép số.
pub const MASK_PRIORITY: u64 = CONFIGURE_MASK_PRIORITY;
/// Mask `vspace?` (bit 1).
pub const MASK_VSPACE: u64 = CONFIGURE_MASK_VSPACE;
/// Mask `entry?` (bit 2).
pub const MASK_ENTRY: u64 = CONFIGURE_MASK_ENTRY;

/// `MessageInfo::new(label, 0, false, 0, length).pack()` — `0` nếu vi phạm biên (không thể xảy ra
/// với hằng của ảnh; nếu xảy ra, stub gửi `MessageInfo` sai và **mọi** TAP core sẽ đỏ, không im).
const fn msg_info(label: u32, length: u8) -> u64 {
    match MessageInfo::new(label, 0, false, 0, length) {
        Ok(info) => info.pack(),
        Err(_) => 0,
    }
}

/// `u32::MAX` = chưa có TCB; `u64::MAX` = chưa có giá trị.
const NO_TCB: u32 = u32::MAX;
const NO_U64: u64 = u64::MAX;

// ---------------------------------------------------------------------------------------------
// Trạng thái của rig + bằng chứng ghi trong lúc chạy (kernel đọc lại ở `finish`)
// ---------------------------------------------------------------------------------------------

/// TCB đang chạy U-mode (đổi từ TCB thứ nhất sang TCB core ở pha core) — handler `svc` đọc.
static CORE_TCB: AtomicU32 = AtomicU32::new(NO_TCB);
/// CNode gốc của rig (CSpace của mọi TCB).
static ROOT: AtomicU32 = AtomicU32::new(NO_TCB);
/// Untyped của rig (để `check_bytes`).
static UNTYPED: AtomicU32 = AtomicU32::new(NO_TCB);
/// TCB server của ca (b).
static SERVER: AtomicU32 = AtomicU32::new(NO_TCB);
/// TCB của **pha core** (chương trình EL0 thứ hai).
static MAIN_TCB: AtomicU32 = AtomicU32::new(NO_TCB);
/// TCB của **lượt U-mode đầu** (DebugPutStr/Exit) — chỉ để `Machine` có tác vụ `Running`.
static TCB_TURN1: AtomicU32 = AtomicU32::new(NO_TCB);
/// Pha core đã bật (syscall của pha này được ghi bằng chứng).
static CORE_PHASE: AtomicBool = AtomicBool::new(false);
/// Số syscall đã dispatch **trong pha core**.
static CORE_SYSCALLS: AtomicU64 = AtomicU64::new(0);
/// `error` của từng syscall pha core (chỉ số theo thứ tự gọi).
static CORE_ERROR: [AtomicU64; 8] = [const { AtomicU64::new(NO_U64) }; 8];
/// `value` của từng syscall pha core.
static CORE_VALUE: [AtomicU64; 8] = [const { AtomicU64::new(NO_U64) }; 8];
/// Số lần `dispatch` trả `Switched` (điểm an toàn `Suspend(Defer)` phải làm nó `1`).
static SWITCHED: AtomicU64 = AtomicU64::new(0);
/// Trạng thái TCB **ngay sau** điểm an toàn (`TcbState` dạng số; `u64::MAX` = chưa đo).
static SUSPEND_STATE: AtomicU64 = AtomicU64::new(NO_U64);
/// `Tcb.Resume` trả `entry` bao nhiêu (`0` = lỗi/không có).
static RESUME_ENTRY: AtomicU64 = AtomicU64::new(0);
/// `Tcb.Resume` gọi được (`true` ⇔ invocation trả `Ok`).
static RESUME_OK: AtomicBool = AtomicBool::new(false);
/// Mã thoát cuối của chương trình EL0 (`Exit`, pha core).
static EXIT_CODE: AtomicU64 = AtomicU64::new(NO_U64);
/// **KT-339** — PA của frame **IPC buffer** đã map (K5 bind `Frame` đại diện cho echo).
static BUF_PA: AtomicU64 = AtomicU64::new(0);
/// `CoreReport` của `init` (bằng chứng TAP 36) — gói trong 6 số.
static REPORT_ADDR: AtomicU64 = AtomicU64::new(0);
static REPORT_BYTES: AtomicU64 = AtomicU64::new(0);
static REPORT_PIN: AtomicU64 = AtomicU64::new(0);
static REPORT_HARTS: AtomicU64 = AtomicU64::new(0);
static REPORT_OBJECTS: AtomicU64 = AtomicU64::new(0);
static REPORT_CAPS: AtomicU64 = AtomicU64::new(0);
static REPORT_HEALTHY: AtomicBool = AtomicBool::new(false);

/// **KT-339** — PA của frame IPC buffer (đã map ở [`prepare`]); K5 dùng lại **một** trang cho
/// mọi TCB (ba pha chạy tuần tự, không có hai tiến trình cùng lúc).
pub fn ipc_buffer_pa() -> u64 {
    BUF_PA.load(Ordering::Acquire)
}

/// Đọc TCB đang chạy U-mode (handler `svc`).
pub fn current_tcb() -> Option<ObjId> {
    let tcb = CORE_TCB.load(Ordering::Acquire);
    (tcb != NO_TCB).then_some(tcb)
}

/// `true` ⇔ mọi thứ đã dựng xong (dùng cho `TAP 36`).
pub fn ready() -> bool {
    CORE_TCB.load(Ordering::Acquire) != NO_TCB && REPORT_HEALTHY.load(Ordering::Acquire)
}

// ---------------------------------------------------------------------------------------------
// Dựng rig — gọi ở `high_entry` **trước** lần `eret` đầu tiên (syscall của cả hai lượt U-mode đi
// qua `vifast-core`, nên `Machine` + một TCB `Running` phải có trước lượt 1)
// ---------------------------------------------------------------------------------------------

/// Dựng `Machine`, CSpace và TCB của **lượt U-mode đầu** (DebugPutStr/Exit) + toàn bộ object của
/// pha core; map trang IPC buffer vào `VSpace` thật; công bố biên ảnh cho `core_bridge`.
///
/// Trả `false` ⇔ bất kỳ bước nào hỏng (ảnh in `PANIC` — fail-loud, không chạy tiếp với model
/// nửa vời). Mọi lỗi core đều có mã; rig **không** nuốt lỗi.
pub fn prepare(
    vs: &mut VSpace<SharedBootMemory>,
    text_start_pa: u64,
    console: &mut Console,
) -> bool {
    // ---- 0) Biên vùng ảnh (PA) — chứng minh `static MACHINE` nằm trong ảnh, không phải arena.
    // Đang chạy ở VA nửa cao: `pa = text_start_pa + (va - KERNEL_HIGH_BASE)` (cùng công thức
    // `umode::high_to_pa`).
    let kernel_end_va = core::ptr::addr_of!(crate::__kernel_end) as u64;
    let image_end = text_start_pa + kernel_end_va.saturating_sub(KERNEL_HIGH_BASE);
    core_bridge::set_image_bounds(text_start_pa, image_end);

    // ---- 1) Untyped boot: 4 frame 4 KiB liền nhau từ arena (nhãn PA — D2-83).
    let mut arena_pa = 0u64;
    for index in 0..4 {
        let Some(pa) = bootmem::alloc_boot_frame() else {
            let _ = writeln!(
                console,
                "# core fail: stage=arena index={index} (hết frame)"
            );
            return false;
        };
        if index == 0 {
            arena_pa = pa;
        }
    }

    // ---- 2) Trang IPC buffer: một frame, zero, map `USER_RW` tại `IPC_BUF_VA`.
    let Some(buf_pa) = bootmem::alloc_boot_frame() else {
        let _ = writeln!(console, "# core fail: stage=buffer (hết frame)");
        return false;
    };
    asm::zero_boot_ram(buf_pa, 0x1000);
    BUF_PA.store(buf_pa, Ordering::Release);
    let Some(frame) = Frame::new(buf_pa) else {
        let _ = writeln!(console, "# core fail: stage=buffer frame pa=0x{buf_pa:x}");
        return false;
    };
    if vs
        .map_obj_with(IPC_BUF_VA, frame, Perms::USER_RW, MemAttr::Normal)
        .is_err()
    {
        let _ = writeln!(console, "# core fail: stage=buffer map va=0x{IPC_BUF_VA:x}");
        return false;
    }
    // VA mới chưa từng được dùng nên TLB không có entry cũ; `activate` vẫn gọi để chứng minh
    // `TTBR0` đang là bảng vừa map (cùng khuôn `umode::run`).
    if vs.activate().is_err() {
        let _ = writeln!(console, "# core fail: stage=buffer activate");
        return false;
    }

    // ---- 3) Model core + rig (một `with_machine`, không lồng khoá).
    let harts = crate::smpboot::online_hart_count();
    let report = core_bridge::init(arena_pa, 4 * 0x1000, harts);
    REPORT_ADDR.store(report.addr, Ordering::Release);
    REPORT_BYTES.store(report.static_bytes as u64, Ordering::Release);
    REPORT_PIN.store(report.pin as u64, Ordering::Release);
    REPORT_HARTS.store(report.harts as u64, Ordering::Release);
    REPORT_OBJECTS.store(report.objects as u64, Ordering::Release);
    REPORT_CAPS.store(report.caps as u64, Ordering::Release);
    REPORT_HEALTHY.store(report.healthy(), Ordering::Release);
    if !report.healthy() {
        let _ = writeln!(
            console,
            "# core fail: stage=report addr=0x{:x} size={} pin={} in_image={} caps={} bytes={} all_bytes={} image=[0x{:x},0x{:x})",
            report.addr,
            report.static_bytes,
            report.pin,
            u8::from(report.in_image),
            u8::from(report.check_caps),
            u8::from(report.check_bytes),
            u8::from(report.check_all_bytes),
            text_start_pa,
            image_end,
        );
        return false;
    }

    let built = core_bridge::with_machine(|m| -> Result<(), KError> {
        let untyped = report.untyped;
        UNTYPED.store(untyped, Ordering::Release);
        let root = m.add_cnode(ROOT_SLOTS);
        ROOT.store(root, Ordering::Release);
        // Ô 0 = cap `Untyped` (nguồn Retype của mọi object còn lại).
        let uc = m.root_cap(untyped);
        m.put_cap(root, SLOT_UNTYPED, uc)?;
        let us = SlotRef::new(root, SLOT_UNTYPED);

        // TCB của lượt U-mode đầu (DebugPutStr/Exit) — chỉ cần `Running` + CSpace.
        let tcb_turn1_cap = m.retype(us, ObjType::Tcb, 0, 0)?;
        m.put_cap(root, SLOT_TCB, tcb_turn1_cap)?;
        let tcb_turn1 = m.cap_obj(tcb_turn1_cap)?;
        TCB_TURN1.store(tcb_turn1, Ordering::Release);
        // TCB của **pha core** — tác vụ mà chương trình EL0 thứ hai gọi invocation trên chính nó.
        let tcb_core_cap = m.retype(us, ObjType::Tcb, 0, 0)?;
        m.put_cap(root, SLOT_CORE, tcb_core_cap)?;
        let tcb_core = m.cap_obj(tcb_core_cap)?;
        MAIN_TCB.store(tcb_core, Ordering::Release);
        // TCB server của ca (b).
        let srv_cap = m.retype(us, ObjType::Tcb, 0, 0)?;
        m.put_cap(root, SLOT_SERVER, srv_cap)?;
        let server = m.cap_obj(srv_cap)?;
        SERVER.store(server, Ordering::Release);
        // Endpoint của ca (b).
        let ep_cap = m.retype(us, ObjType::Endpoint, 0, 0)?;
        m.put_cap(root, SLOT_EP, ep_cap)?;
        // Cap `CNode` của **chính** CSpace gốc: `Tcb.Configure` tham số `cspace?` cần cap này.
        let cspace_cap = m.root_cap(root);
        m.put_cap(root, SLOT_CSPACE, cspace_cap)?;
        // Frame IPC buffer: object **đại diện** (D2-83 — object layer sở hữu Frame thật; ở đây
        // "thật" là một frame arena đã map, core chỉ giữ nhãn).
        let frame_obj = m.add_foreign_object(ObjType::Frame, buf_pa)?;
        let frame_cap = m.root_cap(frame_obj);
        m.put_cap(root, SLOT_FRAME, frame_cap)?;
        // VSpace **đại diện** cho ca (c): arch từ chối giữ danh tính ⇒ `Failed(Busy)`.
        let vspace_obj = m.add_foreign_object(ObjType::VSpace, 0x338b_5653)?;
        let vspace_cap = m.root_cap(vspace_obj);
        m.put_cap(root, SLOT_VSPACE, vspace_cap)?;
        // Cap TCB **copy thiếu `RIGHT_WRITE`** của TCB core — ca (c) kiểm quyền.
        let ro = m.copy_rights(
            SlotRef::new(root, SLOT_CORE),
            SlotRef::new(root, SLOT_TCB_RO),
            Some(RIGHT_READ),
        )?;
        // Bất biến biên dịch của ca (c): cap phải **sống** và **thiếu** `RIGHT_WRITE`.
        if !m.cap_info(ro)?.alive || m.cap_info(ro)?.rights & vifast_core::spec::RIGHT_WRITE != 0 {
            return Err(KError::InvalidCapability);
        }

        // Lượt 1: TCB chỉ cần CSpace + `Running` (syscall 12/14 không dùng cap).
        m.tcb_configure(
            SlotRef::new(root, SLOT_TCB),
            Some(SlotRef::new(root, SLOT_CSPACE)),
        )?;
        m.start_tcb(tcb_turn1)?;
        m.transition_tcb(tcb_turn1, NonReadyTarget::Running { hart: 0 })?;
        CORE_TCB.store(tcb_turn1, Ordering::Release);
        // Pha core: TCB có CSpace + IPC buffer + ưu tiên ban đầu; **chưa** chạy (chạy ở `start`).
        m.tcb_configure_with_ipc_buffer(
            SlotRef::new(root, SLOT_CORE),
            Some(SlotRef::new(root, SLOT_CSPACE)),
            Some((SlotRef::new(root, SLOT_FRAME), IPC_BUF_VA)),
        )?;
        m.tcb_set_priority(SlotRef::new(root, SLOT_CORE), PRIORITY_OLD, u8::MAX)?;
        // TCB server: cùng CSpace; chạy + `Recv` ở `start` (sau khi mọi ca 1..35 xong).
        m.tcb_configure(
            SlotRef::new(root, SLOT_SERVER),
            Some(SlotRef::new(root, SLOT_CSPACE)),
        )?;
        m.start_tcb(server)?;
        m.transition_tcb(server, NonReadyTarget::Running { hart: 0 })?;
        Ok(())
    });
    if built.is_err() {
        let _ = writeln!(
            console,
            "# core fail: addr=0x{:x} size={} in_image={} healthy={} harts={} image=[0x{:x},0x{:x})",
            report.addr,
            report.static_bytes,
            u8::from(report.in_image),
            u8::from(report.healthy()),
            report.harts,
            text_start_pa,
            image_end,
        );
    }
    built.is_ok()
}

/// In **một** dòng `# core:` (bằng chứng máy kiểm, không phải lời văn) + `# core ipc:`.
pub fn report(console: &mut Console) {
    // Đọc **sống** số object/cap của cả rig (không phải số của riêng `init`): một dòng `# core:`
    // mà `objects=1 caps=0` sẽ nói dối về model đã dựng.
    let (objects, caps) = core_bridge::with_machine(|m| (m.object_count(), m.live_cap_count()));
    let _ = writeln!(
        console,
        "# core: addr=0x{:x} size={} pin={} in_image={} harts={} objects={} caps={} healthy={}",
        REPORT_ADDR.load(Ordering::Acquire),
        REPORT_BYTES.load(Ordering::Acquire),
        REPORT_PIN.load(Ordering::Acquire),
        if core_bridge::in_image_region(
            REPORT_ADDR.load(Ordering::Acquire),
            REPORT_BYTES.load(Ordering::Acquire) as usize
        ) {
            1
        } else {
            0
        },
        REPORT_HARTS.load(Ordering::Acquire),
        objects,
        caps,
        if REPORT_HEALTHY.load(Ordering::Acquire) {
            1
        } else {
            0
        },
    );
    let (mr, extra, configure) = vifast_plat_aarch64::syscall_env::LAYOUT_EVIDENCE;
    let _ = writeln!(
        console,
        "# core buffer: va=0x{IPC_BUF_VA:x} mr_offset={mr} extra_offset={extra} configure_offset={configure} harts={}",
        core_bridge::harts(),
    );
    let _ = writeln!(
        console,
        "# core lock: acquired={} contended={} in_cs={} overlap={}",
        core_bridge::lock_stats().acquired,
        core_bridge::lock_stats().contended,
        core_bridge::lock_stats().in_cs,
        core_bridge::lock_stats().overlap,
    );
}

// ---------------------------------------------------------------------------------------------
// Pha core — chương trình EL0 thứ hai (sau khi TAP 1..35 đã xanh)
// ---------------------------------------------------------------------------------------------

/// Điền **header IPC buffer** (magic + window + vùng extra caps + vùng `Tcb.Configure`) và
/// MR4..MR7 — **trước** mỗi lần `eret` vào pha core.
///
/// Vì sao kernel điền hộ: ca (b) kiểm **arch đọc đúng bộ nhớ user tại VA đã bind**; nếu kernel
/// không điền thì không có gì để đọc. Cùng khuôn `KT-282` phía riscv64 (rig ghi buffer trước khi
/// chạy client). Giá trị chọn **không** trùng nhau để một lần đọc lệch offset là lộ ngay.
fn fill_user_buffer(priority: u8, vspace_cptr: u64, entry: u64) {
    let base = IPC_BUF_VA;
    let set = |offset: usize, value: u64| {
        asm::write_u64(base + offset as u64, value);
    };
    // Header 10 word: magic, info, receive window 4 CPTR, 4 CPTR nguồn cap.
    set(0, IPC_HEADER_MAGIC);
    set(8, 0);
    // Window 4 ô **phân biệt**, đều rỗng trong CSpace 16 ô (core chỉ kiểm chỉ số hợp lệ).
    for (index, slot) in [8u64, 9, 10, 11].iter().enumerate() {
        set(16 + index * 8, *slot);
    }
    for index in 0..4 {
        set(48 + index * 8, 0);
    }
    // Vùng extra caps (560): n = 0, 4 CPTR = 0, dst = 0.
    for index in 0..6 {
        set(IPC_BUFFER_EXTRA_CAPS_OFFSET + index * 8, 0);
    }
    // Vùng `Tcb.Configure` (608): cspace, vspace, entry, priority.
    set(IPC_BUFFER_CONFIGURE_OFFSET, 0);
    set(IPC_BUFFER_CONFIGURE_OFFSET + 8, vspace_cptr);
    set(IPC_BUFFER_CONFIGURE_OFFSET + 16, entry);
    set(IPC_BUFFER_CONFIGURE_OFFSET + 24, u64::from(priority));
    // MR4..MR7 của message ca (b).
    for (index, word) in [MSG_MR4, MSG_MR5, MSG_MR6, MSG_MR7].iter().enumerate() {
        set(IPC_BUFFER_MR_OFFSET + index * 8, *word);
    }
}

/// Bắt đầu pha core: điền buffer, bật server `Recv`, đổi TCB hiện hành, rồi `eret` xuống EL0.
///
/// **Không trả về** — đường `Exit` của pha core `eret` về [`finish`] (EL1h).
pub fn start(console: &mut Console) -> ! {
    // Điểm vào `__el0_after_suspend` — **VA user** (trang mã EL0) tính từ PA symbol.
    let entry = crate::umode::core_after_suspend_va();
    if entry == 0 {
        let _ = writeln!(
            console,
            "PANIC: coretest: không đọc được điểm vào sau Suspend"
        );
        debug_halt(3)
    }
    let vspace_cptr = SLOT_VSPACE as u64;
    fill_user_buffer(PRIORITY_SET, vspace_cptr, entry);
    vifast_plat_aarch64::syscall_env::set_ipc_buffer_va(Some(IPC_BUF_VA));

    let tcb = MAIN_TCB.load(Ordering::Acquire);
    let switched = core_bridge::with_machine(|m| -> Result<(), KError> {
        let root = ROOT.load(Ordering::Acquire);
        let server = SERVER.load(Ordering::Acquire);
        // TCB core: `Inactive` → `start_tcb` (`Ready`) → `Running` (chạy chương trình EL0 thứ hai).
        if matches!(m.tcb_state(tcb), Ok(TcbState::Inactive)) {
            m.start_tcb(tcb)?;
        }
        m.transition_tcb(tcb, NonReadyTarget::Running { hart: 0 })
            .map_err(|e| {
                let _ = writeln!(console, "# core fail: stage=transition err={e:?}");
                e
            })?;
        // Server chặn `Recv` **trước** khi client gửi ⇒ ca (b) giao ngay, không cần Reply.
        m.recv(server, SlotRef::new(root, SLOT_EP), None)
            .map_err(|e| {
                let _ = writeln!(
                    console,
                    "# core fail: stage=recv err={e:?} server_state={:?}",
                    m.tcb_state(server).map(state_name_of).unwrap_or("none")
                );
                e
            })?;
        Ok(())
    });
    if switched.is_err() {
        let _ = writeln!(console, "PANIC: coretest: rig Recv lỗi");
        debug_halt(3)
    }
    set_current(tcb);
    CORE_PHASE.store(true, Ordering::Release);
    crate::umode::run_core_phase(console)
}

/// Gọi từ `core_switched` (EL1h) sau khi `dispatch` trả `Switched` (điểm an toàn của
/// `Suspend(Defer)`): đo trạng thái, `Tcb.Resume` (method 12) rồi `eret` lại EL0 tại **điểm vào
/// do core giao** (D2-83: core không ghi `ELR_EL1`).
pub fn resume_after_switch(console: &mut Console) -> ! {
    let tcb = current_tcb().unwrap_or(NO_TCB);
    SWITCHED.fetch_add(1, Ordering::Relaxed);
    // `Tcb.Resume` đi qua **đúng** đường invocation (`invoke::dispatch`) — không gọi tắt model
    // API: như vậy phép kiểm phủ luôn kiểm quyền + kiểm trạng thái + hook `SyscallEnv::tcb_resume`
    // (D2-83: core giao `entry`, arch tiêu thụ).
    let (state, entry) = core_bridge::with_machine(|m| {
        let state = m.tcb_state(tcb).map(|s| state_code(s)).unwrap_or(NO_U64);
        SUSPEND_STATE.store(state, Ordering::Release);
        let info = match MessageInfo::new(vifast_core::invoke::TCB_RESUME, 0, false, 0, 0) {
            Ok(info) => info.pack(),
            Err(_) => 0,
        };
        let result = invoke::dispatch(
            m,
            tcb,
            SlotRef::new(ROOT.load(Ordering::Acquire), SLOT_CORE),
            info,
            &[0; 4],
            &mut vifast_plat_aarch64::syscall_env::Aarch64Env,
        );
        match result {
            Ok((0, entry)) => (state, Ok(entry)),
            Ok((error, value)) => (state, Err((error, value))),
            Err(error) => (state, Err((u64::from(error), 0))),
        }
    });
    let Ok(entry) = entry else {
        let _ = writeln!(
            console,
            "PANIC: coretest: Tcb.Resume lỗi state={state} err={entry:?}"
        );
        debug_halt(4)
    };
    RESUME_ENTRY.store(entry, Ordering::Release);
    // Arch **tiêu thụ** `entry` do `Tcb.Resume` giao qua hook `SyscallEnv::tcb_resume`.
    let Some(pc) = vifast_plat_aarch64::syscall_env::take_resume(tcb) else {
        let _ = writeln!(console, "PANIC: coretest: Tcb.Resume không giao entry");
        debug_halt(4)
    };
    RESUME_OK.store(pc == entry, Ordering::Release);
    let _ = writeln!(
        console,
        "# core resume: tcb={tcb} state_after_switch=0x{state:x} entry=0x{entry:x} match={}",
        if pc == entry { 1 } else { 0 },
    );
    asm::eret_to_el0(entry, EL0_STACK_TOP, crate::umode::spsr_el0_phase())
}

/// `TcbState` → số ổn định để ghi log (không dùng `{:?}` vì log cần so được bằng máy).
pub fn state_code(state: TcbState) -> u64 {
    match state {
        TcbState::Inactive => 0,
        TcbState::Ready => 1,
        TcbState::Running { .. } => 2,
        TcbState::BlockedOnSend { .. } => 3,
        TcbState::BlockedOnRecv { .. } => 4,
        TcbState::BlockedOnReply { .. } => 5,
        TcbState::BlockedOnNotification { .. } => 6,
        TcbState::BlockedOnAddress { .. } => 7,
        TcbState::Sleeping { .. } => 8,
        TcbState::Stopping => 9,
        TcbState::Dead => 10,
    }
}

/// Tên trạng thái cho log (`# core suspend:`).
fn state_name(code: u64) -> &'static str {
    match code {
        0 => "Inactive",
        1 => "Ready",
        2 => "Running",
        3 => "BlockedOnSend",
        4 => "BlockedOnRecv",
        5 => "BlockedOnReply",
        6 => "BlockedOnNotification",
        7 => "BlockedOnAddress",
        8 => "Sleeping",
        9 => "Stopping",
        10 => "Dead",
        _ => "?",
    }
}

/// Số khe ghi bằng chứng syscall của pha core (0 = Configure ưu tiên, 1 = Send, 2 = Configure
/// quyền, 3 = Configure vspace, 4 = Configure entry, 5 = Suspend).
pub const CORE_SYSCALL_RECORDS: usize = 8;

/// Ghi kết quả **một** syscall của pha core (handler gọi) — chỉ số = thứ tự gọi, không do EL0
/// khai.
pub fn record_syscall(error: u64, value: u64) {
    let slot = CORE_SYSCALLS.fetch_add(1, Ordering::Release);
    if (slot as usize) < CORE_SYSCALL_RECORDS {
        CORE_ERROR[slot as usize].store(error, Ordering::Release);
        CORE_VALUE[slot as usize].store(value, Ordering::Release);
    }
}

/// Ghi mã thoát của pha core (`Exit`).
pub fn record_exit(code: u64) {
    EXIT_CODE.store(code, Ordering::Release);
}

/// Điểm hội tụ của pha core: đo lại toàn bộ bằng chứng, phát TAP 36..41, marker + halt.
///
/// **Không trả về** (kết thúc ảnh kiểm như `el0_done` của lượt 1).
pub fn finish(console: &mut Console) -> ! {
    // ---- Khoá/ngắt (TAP 41): cần `DAIF.I` **đang mở** để phép đo "đã khôi phục" có nghĩa.
    let (irq_before, irq_inside, irq_after) = {
        // `msr daifclr,#2` — mở IRQ cho hart này. `smpboot` đã mở trước đó; gọi lại để phép đo
        // không phụ thuộc thứ tự khởi tạo của đường U-mode.
        asm::daif_clear_irq();
        let before = vifast_plat_aarch64::irq_lock::irq_enabled();
        let mut inside = true;
        core_bridge::with_machine(|_| {
            inside = vifast_plat_aarch64::irq_lock::irq_enabled();
        });
        (before, inside, vifast_plat_aarch64::irq_lock::irq_enabled())
    };
    let lock = core_bridge::lock_stats();
    let lock_ok = irq_before && !irq_inside && irq_after && lock.ok();

    // ---- Đo trạng thái cuối của `Machine` (mọi kết luận lấy từ đây, không từ lời chương trình).
    let mut facts = core_bridge::with_machine(|m| Facts {
        priority: m
            .tcb_info(SlotRef::new(ROOT.load(Ordering::Acquire), SLOT_CORE))
            .map(|i| i.priority)
            .unwrap_or(0),
        tcb_state: m.tcb_state(MAIN_TCB.load(Ordering::Acquire)).ok(),
        vspace: m.tcb_vspace(MAIN_TCB.load(Ordering::Acquire)).unwrap_or(0),
        server_state: m.tcb_state(SERVER.load(Ordering::Acquire)).ok(),
        area: m.tcb_ipc_area(SERVER.load(Ordering::Acquire)).ok(),
        caps_ok: m.check_caps().is_ok(),
        bytes_ok: m.check_bytes(UNTYPED.load(Ordering::Acquire)).is_ok(),
        all_bytes_ok: m.check_all_bytes().is_ok(),
        ipc_ok: m.check_ipc().is_ok(),
        objects: m.object_count(),
        caps: m.live_cap_count(),
        configure_stat: 0,
        deferred_stat: 0,
        resume_stat: 0,
    });
    let stats = invoke::stats();
    facts.configure_stat = u64::from(stats.configure);
    facts.deferred_stat = u64::from(stats.deferred);
    facts.resume_stat = u64::from(stats.resume);

    // ---- Bằng chứng dạng dòng `#` (máy kiểm được, không phải trang trí).
    report(console);
    let _ = writeln!(
        console,
        "# core configure: priority_old={PRIORITY_OLD} priority_now={} err0={} err_ro={} err_vspace={} err_entry={} configure={}",
        facts.priority,
        field(&CORE_ERROR, 0),
        field(&CORE_ERROR, 2),
        field(&CORE_ERROR, 3),
        field(&CORE_ERROR, 4),
        facts.configure_stat,
    );
    let (msg_len, msg_mr4, msg_mr7) = match &facts.area {
        Some(area) => match area.info {
            Some(info) => (u64::from(info.length()), area.buffer[0], area.buffer[3]),
            None => (0, 0, 0),
        },
        None => (0, 0, 0),
    };
    let _ = writeln!(
        console,
        "# core ipc: send_err={} len={} mr0=0x{:x} mr3=0x{:x} mr4=0x{:x} mr7=0x{:x} server={}",
        field(&CORE_ERROR, 1),
        msg_len,
        facts.area.as_ref().map(|a| a.regs[0]).unwrap_or(0),
        facts.area.as_ref().map(|a| a.regs[3]).unwrap_or(0),
        msg_mr4,
        msg_mr7,
        facts.server_state.map(state_name_of).unwrap_or("none"),
    );
    let _ = writeln!(
        console,
        "# core suspend: switched={} state={} resume_entry=0x{:x} resume_ok={} exit={} deferred={} resume_stat={}",
        SWITCHED.load(Ordering::Acquire),
        state_name(SUSPEND_STATE.load(Ordering::Acquire)),
        RESUME_ENTRY.load(Ordering::Acquire),
        if RESUME_OK.load(Ordering::Acquire) { 1 } else { 0 },
        EXIT_CODE.load(Ordering::Acquire),
        facts.deferred_stat,
        facts.resume_stat,
    );
    let _ = writeln!(
        console,
        "# core lock: irq_before={} irq_inside={} irq_after={} acquired={} overlap={} in_cs={} ok={}",
        u8::from(irq_before),
        u8::from(irq_inside),
        u8::from(irq_after),
        lock.acquired,
        lock.overlap,
        lock.in_cs,
        u8::from(lock_ok),
    );

    // ---- TAP 36..41 — mỗi ca một điều kiện **đo được**, không có "ok" mặc định.
    let mut all = true;
    all &= tap(
        console,
        36,
        "core: Machine linked in image, pin + invariants",
        ready()
            && core_bridge::in_image_region(
                REPORT_ADDR.load(Ordering::Acquire),
                REPORT_BYTES.load(Ordering::Acquire) as usize,
            )
            && REPORT_BYTES.load(Ordering::Acquire) <= REPORT_PIN.load(Ordering::Acquire),
    );
    all &= tap(
        console,
        37,
        "core: Tcb.Configure priority from EL0",
        field(&CORE_ERROR, 0) == 0 && facts.priority == PRIORITY_SET,
    );
    let mr_match = facts.area.as_ref().is_some_and(|area| {
        area.info.is_some()
            && u64::from(area.info.unwrap().length()) == 8
            && area.regs == [MSG_MR0, MSG_MR1, MSG_MR2, MSG_MR3]
            && area.buffer[0] == MSG_MR4
            && area.buffer[1] == MSG_MR5
            && area.buffer[2] == MSG_MR6
            && area.buffer[3] == MSG_MR7
    });
    all &= tap(
        console,
        38,
        "ipc: el0 send 8 MR -> endpoint recv (MR4..MR7 buffer)",
        field(&CORE_ERROR, 1) == 0
            && mr_match
            && matches!(facts.server_state, Some(TcbState::Ready)),
    );
    all &= tap(
        console,
        39,
        "core: Configure rights=3, vspace=14 rollback (no panic)",
        field(&CORE_ERROR, 2) == u64::from(KError::InsufficientRights.id())
            && field(&CORE_ERROR, 3) == u64::from(KError::Busy.id())
            && facts.vspace == 0,
    );
    all &= tap(
        console,
        40,
        "core: Tcb.Suspend(Defer) safe point + Resume entry",
        SWITCHED.load(Ordering::Acquire) == 1
            && SUSPEND_STATE.load(Ordering::Acquire) == state_code(TcbState::Inactive)
            && SUSPEND_STATE.load(Ordering::Acquire) != NO_U64
            && RESUME_OK.load(Ordering::Acquire)
            && RESUME_ENTRY.load(Ordering::Acquire) != 0
            && field(&CORE_ERROR, 4) == 0
            && EXIT_CODE.load(Ordering::Acquire) == 0
            && facts.deferred_stat == 1,
    );
    all &= tap(
        console,
        41,
        "lock: DAIF.I masked in critical section, restored after",
        lock_ok,
    );

    // ---- Bất biến cuối: cap/byte/IPC của core phải xanh; sai ⇒ in rõ, không "ok" giả.
    if !(facts.caps_ok && facts.bytes_ok && facts.all_bytes_ok && facts.ipc_ok) {
        let _ = writeln!(
            console,
            "# core check: caps={} bytes={} all_bytes={} ipc={} objects={} live_caps={} tcb_state={}",
            u8::from(facts.caps_ok),
            u8::from(facts.bytes_ok),
            u8::from(facts.all_bytes_ok),
            u8::from(facts.ipc_ok),
            facts.objects,
            facts.caps,
            facts.tcb_state.map(state_name_of).unwrap_or("none"),
        );
        all = false;
    }
    if !all {
        debug_halt(5)
    }
    // **KT-339** — K5: `procsrv`/`shell`/`echo` **thật** ở EL0 (TAP 42..44). `k5user::start`
    // in TAP của nó rồi marker + halt (cùng bất biến "TAP đỏ ⇒ không marker").
    crate::k5user::start(console)
}

/// Đọc `CORE_ERROR[i]` (`NO_U64` ⇒ in ra số lớn — luôn khác mọi mã lỗi thật ⇒ TAP đỏ).
fn field(array: &[AtomicU64; 8], index: usize) -> u64 {
    array[index].load(Ordering::Acquire)
}

/// Ghi TCB hiện hành (gọi ở `start`): handler `svc` + `finish` cùng đọc một nguồn.
pub fn set_current(tcb: ObjId) {
    MAIN_TCB.store(tcb, Ordering::Release);
    CORE_TCB.store(tcb, Ordering::Release);
}

/// Tên trạng thái của `TcbState` (cho `# core ipc:`).
fn state_name_of(state: TcbState) -> &'static str {
    state_name(state_code(state))
}

/// Trạng thái gói lại từ `with_machine` (không giữ tham chiếu ra ngoài khoá).
struct Facts {
    priority: u8,
    tcb_state: Option<TcbState>,
    vspace: ObjId,
    server_state: Option<TcbState>,
    area: Option<vifast_core::ipc::IpcArea>,
    caps_ok: bool,
    bytes_ok: bool,
    all_bytes_ok: bool,
    ipc_ok: bool,
    objects: usize,
    caps: usize,
    configure_stat: u64,
    deferred_stat: u64,
    resume_stat: u64,
}

const _: () = assert!(ROOT_SLOTS <= 1 << 16);
const _: () = assert!(WORDS_IN_REGISTERS == 4);
/// `TcbState` có 11 biến thể — `state_code` phải phủ hết (lệch là lỗi biên dịch).
const _: () = assert!(TCB_STATE_COUNT == 11);
/// `TAP_TESTS` phải khớp số TAP mà ảnh phát ra (KT-338b: 35 + 6; KT-339: + 3 ca K5).
const _: () = assert!(TAP_TESTS == 44);
/// Import thuộc về chữ ký `SyscallEnv`/`MessageInfo` — dùng để kiểm biên dịch (không chết).
const _: fn(u32, u8) -> u64 = |label, len| match MessageInfo::new(label, 0, false, 0, len) {
    Ok(info) => info.pack(),
    Err(_) => 0,
};
const _: u64 = CONFIGURE_MASK_PRIORITY | CONFIGURE_MASK_VSPACE | CONFIGURE_MASK_ENTRY;
const _: u64 = SUSPEND_MODE_DEFER;
const _: u8 = RIGHTS_ALL;
const _: fn() = || {
    let _ = <vifast_plat_aarch64::syscall_env::Aarch64Env as SyscallEnv>::features;
    let _ = SyscallOutcome::Exit { code: 0 };
};
