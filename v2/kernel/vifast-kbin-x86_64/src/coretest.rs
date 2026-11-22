// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-340c** — ảnh kiểm x86_64: `vifast-core::Machine` + IPC từ **ring 3** qua `int 0x80`.
//!
//! # Vì sao tệp này tồn tại (và vì sao nó **không** nằm trong ảnh sản phẩm)
//!
//! `KT-340b` đã có `VSpace` **động** và ring 3 thật, `KT-340d` đã có TLB shootdown + `IpiMailbox`.
//! `KT-340c` nối **model core** (`vifast-core::Machine`, 184.792 B) vào ảnh x86_64 để mọi
//! `int 0x80` đi qua **một** `vifast_core::syscall::dispatch` (D2-79) và `MessageInfo` trả về nằm
//! ở **`r13`** (`7C-r13`, D2-242). Model nằm trong `.bss` **ảnh kiểm**; ảnh sản phẩm **không**
//! link nó (trần `.data+.bss` sản phẩm 65.536 B — D2-243/D2-244). `KT-341` (shell sản phẩm) là
//! task quyết định ngân sách để mang core vào ảnh sản phẩm.
//!
//! # Năm ca kiểm của task (TAP 38..43) — đo ở **kernel**, không tin lời chương trình
//!
//! | TAP | Ca | Bằng chứng kernel |
//! |---|---|---|
//! | 38 | core link | `size_of::<Machine>()` ≤ pin 196.608 + static **trong ảnh** + `check_caps`/`check_bytes`/`check_all_bytes` xanh |
//! | 39 | (a) `Tcb.Configure` đổi **ưu tiên** + `VspaceOutcome` | `priority` = 9 sau invocation từ ring 3 (trước = 4); `vspace?` ⇒ arch `Failed(Busy)` 14 + rollback 0 |
//! | 40 | (b) gửi/nhận **1 message** qua Endpoint, MR4..MR7 | `tcb_ipc_area(server)` có `length = 8`, MR0..3 = hằng của client, MR4..7 = hằng **chỉ có** trong buffer |
//! | 41 | (d) **`7C-r13`** | `dispatch` trả `MessageInfo` ở `r13` và MR3 ở `r12`; `Recv` từ EP2 (sender đã blocked) trả info/length đúng |
//! | 42 | (c) `Tcb.Suspend(Defer)` | điểm an toàn ⇒ `dispatch` trả `Switched`, TCB `Inactive`; `Tcb.Resume` trả `entry` ⇒ ring 3 chạy tiếp **đúng** điểm vào đó |
//! | 43 | khoá `IrqMcsLock` | `RFLAGS.IF` **tắt** trong vùng găng, **khôi phục** sau khi nhả (`acquired > 0`, `overlap = 0`, `in_cs = 0`) |
//!
//! # Vì sao **kernel** là nơi khẳng định, không phải chương trình ring 3
//!
//! Chương trình ring 3 chỉ *gọi* syscall; mọi kết luận lấy từ trạng thái `Machine` (qua
//! `core_bridge::with_machine`) — nếu chương trình in "ok" mà core không đổi gì thì TAP vẫn đỏ.
//! Phần **duy nhất** kernel không đọc hộ được là **thanh ghi** `r13`/`r12` của chương trình sau
//! `iretq`: chương trình tự so rồi gộp kết quả vào **mã thoát**; kernel đọc mã thoát **và** đối
//! chiếu với vùng nhận của TCB (hai nguồn độc lập). Cùng nguyên tắc `KT-319`/`KT-320` của
//! riscv64 và `KT-338b` của aarch64 (D2-102: in từ dữ liệu, không chép chuỗi).
//!
//! # Đột biến (chỉ để chứng minh bộ kiểm có răng — D2-05)
//!
//! - `mut-info-r12` (feature của ảnh): `syscall_dispatch` ghi `MessageInfo` trả về vào **`r12`**
//!   thay `r13` (và MR3 vào `r13`) ⇒ chương trình ring 3 đọc `r13` **không** thấy info, `r12`
//!   **không** còn MR3 ⇒ TAP **41** ĐỎ. Đây là **đột biến (i)** bắt buộc của `KT-340c`.
//! - `mut-no-irq-restore` (feature của crate arch): `IrqState::restore` **bỏ** `sti` ⇒ TAP **43**
//!   ĐỎ (`irq_after=0`). Đây là **đột biến (ii)**.

use core::arch::global_asm;
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use vifast_core::invoke::{self, SUSPEND_MODE_DEFER};
use vifast_core::ipc::{
    MessageInfo, CONFIGURE_MASK_ENTRY, CONFIGURE_MASK_PRIORITY, CONFIGURE_MASK_VSPACE,
    IPC_BUFFER_CONFIGURE_OFFSET, IPC_BUFFER_EXTRA_CAPS_OFFSET, IPC_BUFFER_MR_OFFSET,
    IPC_HEADER_MAGIC, WORDS_IN_IPC_BUFFER,
};
use vifast_core::syscall::{SyscallEnv, SyscallOutcome};
use vifast_core::{
    KError, Message, NonReadyTarget, ObjId, ObjType, SlotRef, TcbState, TCB_STATE_COUNT,
};

use vifast_plat_x86_64::bootmem::BootArena;
use vifast_plat_x86_64::core_bridge;
use vifast_plat_x86_64::format::Perms;
use vifast_plat_x86_64::irq_lock::irq_enabled;
use vifast_plat_x86_64::mmu::TableMemory;
use vifast_plat_x86_64::syscall_env::{self, X86_64Env};
use vifast_plat_x86_64::tss;
use vifast_plat_x86_64::umode::{
    USER_RFLAGS, VSPACE_BASE, VSPACE_CODE_VA, VSPACE_STACK_TOP, VSPACE_STACK_VA,
};
use vifast_plat_x86_64::vspace::VSpace;
use vifast_plat_x86_64::{asm, umode};

use crate::{check, Console};

/// Trang **IPC buffer** của pha core — `base + 0x5000` (sau `peer` 0x3000 và `probe` 0x4000 của
/// `KT-340b`; **không** đè trang guard `base + 0x2000`).
pub const IPC_BUF_VA: u64 = VSPACE_BASE + 0x5000;

/// Số ô của CSpace gốc của rig.
const ROOT_SLOTS: usize = 16;
/// Ô cap `Untyped` (nguồn Retype).
pub const SLOT_UNTYPED: usize = 0;
/// Ô cap `Tcb` của tác vụ **lượt 1** (DebugPutStr/Exit).
pub const SLOT_TCB: usize = 1;
/// Ô cap `Tcb` của tác vụ **server** (kernel giữ; `Recv` trên Endpoint).
pub const SLOT_SERVER: usize = 2;
/// Ô cap `Endpoint` — kênh IPC của ca (b).
pub const SLOT_EP: usize = 3;
/// Ô cap `Frame` 4 KiB làm IPC buffer.
pub const SLOT_FRAME: usize = 4;
/// Ô cap **đại diện** `VSpace` (ngoại lai, D2-83) — ca (c) kiểm `VspaceOutcome`.
pub const SLOT_VSPACE: usize = 5;
/// Ô cap `CNode` của chính CSpace gốc.
pub const SLOT_CSPACE: usize = 6;
/// Ô cap `Tcb` của **tác vụ pha core** — tác vụ gọi invocation trên chính nó.
pub const SLOT_CORE: usize = 7;
/// Ô cap `Tcb` của **người gửi** bị chặn `Send` trên EP2 (nguồn message cho ca (d)).
pub const SLOT_SENDER: usize = 8;
/// Ô cap `Endpoint` thứ hai — kênh **nhận** của ca (d).
pub const SLOT_EP2: usize = 9;

/// Ưu tiên **trước** khi `Tcb.Configure` đổi (kernel đặt lúc dựng rig).
const PRIORITY_OLD: u8 = 4;
/// Ưu tiên **sau** khi `Tcb.Configure` đổi — chương trình ring 3 yêu cầu trị này qua IPC buffer.
const PRIORITY_SET: u8 = 9;

/// MR0..MR3 của message ca (b) — chỉ đi trong **thanh ghi**.
pub const MSG_MR0: u64 = 0x340c_0000;
pub const MSG_MR1: u64 = 0x340c_0001;
pub const MSG_MR2: u64 = 0x340c_0002;
pub const MSG_MR3: u64 = 0x340c_0003;
/// MR4..MR7 — chỉ có trong **IPC buffer** (offset [`IPC_BUFFER_MR_OFFSET`] = 80 B).
const MSG_MR4: u64 = 0x340c_0004;
const MSG_MR5: u64 = 0x340c_0005;
const MSG_MR6: u64 = 0x340c_0006;
const MSG_MR7: u64 = 0x340c_0007;

/// Label của message ca (b).
pub const MSG_LABEL: u32 = 0x340;
/// Label của message ca (d) — kernel gửi tới EP2 **trước** khi ring 3 `Recv`.
pub const MSG_RECV_LABEL: u32 = 0x341;
/// MR0..MR3 của message ca (d) — kiểm kênh trả `r10/r8/r9/r12`.
pub const MSG_RECV0: u64 = 0x3410_0000;
pub const MSG_RECV1: u64 = 0x3410_0001;
pub const MSG_RECV2: u64 = 0x3410_0002;
pub const MSG_RECV3: u64 = 0x3410_0003;
/// Số MR của message ca (d) — `length = 8` (MR4..7 nằm trong buffer, không kiểm ở kênh thanh ghi).
const MSG_RECV_WORDS: usize = 8;

/// `MessageInfo` của `Call` với `Tcb.Configure` (`length = 3`).
pub const INFO_CFG3: u64 = msg_info(vifast_core::invoke::TCB_CONFIGURE, 3);
/// `MessageInfo` của message ca (b) — label riêng, `length = 8`.
pub const INFO_SEND: u64 = msg_info(MSG_LABEL, 8);
/// `MessageInfo` của `Tcb.Suspend` (`length = 1` — mode).
pub const INFO_SUSPEND: u64 = msg_info(vifast_core::invoke::TCB_SUSPEND, 1);
/// `MessageInfo` **đã đóng gói** của message ca (d) — chương trình ring 3 so với `r13`.
pub const INFO_RECV: u64 = msg_info(MSG_RECV_LABEL, MSG_RECV_WORDS as u8);
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
/// Số khe ghi bằng chứng syscall của pha core.
pub const CORE_SYSCALL_RECORDS: usize = 8;

// ---------------------------------------------------------------------------------------------
// Trạng thái của rig + bằng chứng ghi trong lúc chạy (kernel đọc lại ở `finish`)
// ---------------------------------------------------------------------------------------------

/// `Tcb` hiện hành của đường syscall (lượt 1 hoặc pha core) — một nguồn cho handler + `finish`.
static CURRENT_TCB: AtomicU32 = AtomicU32::new(NO_TCB);
/// `Tcb` của tác vụ pha core.
static MAIN_TCB: AtomicU32 = AtomicU32::new(NO_TCB);
/// `Tcb` của server (ca (b)).
static SERVER: AtomicU32 = AtomicU32::new(NO_TCB);
/// `Tcb` của người gửi bị chặn (ca (d)).
static SENDER: AtomicU32 = AtomicU32::new(NO_TCB);
/// CSpace gốc của rig (nhãn `ObjId`).
static ROOT: AtomicU32 = AtomicU32::new(NO_TCB);
/// `ObjId` của Untyped boot.
static UNTYPED: AtomicU32 = AtomicU32::new(NO_TCB);
/// Đang ở **pha core** (chương trình ring 3 thứ hai).
static CORE_PHASE: AtomicBool = AtomicBool::new(false);
/// Số lần `dispatch` trả `Switched` (điểm an toàn `Suspend(Defer)`).
static SWITCHED: AtomicU64 = AtomicU64::new(0);
/// Trạng thái TCB ngay sau `Switched` (`state_code`, `NO_U64` = chưa đo).
static SUSPEND_STATE: AtomicU64 = AtomicU64::new(NO_U64);
/// Điểm vào do `Tcb.Resume` trả.
static RESUME_ENTRY: AtomicU64 = AtomicU64::new(0);
/// `true` ⇔ `entry` do `Tcb.Resume` trả **trùng** điểm vào arch nhận qua hook `tcb_resume`.
static RESUME_OK: AtomicBool = AtomicBool::new(false);
/// Mã thoát của pha core (`Exit`).
static EXIT_CODE: AtomicU64 = AtomicU64::new(NO_U64);
/// Số syscall đã ghi bằng chứng.
static CORE_SYSCALLS: AtomicU64 = AtomicU64::new(0);
/// Lỗi của từng syscall (theo thứ tự gọi).
static CORE_ERROR: [AtomicU64; CORE_SYSCALL_RECORDS] =
    [const { AtomicU64::new(NO_U64) }; CORE_SYSCALL_RECORDS];
/// Giá trị trả của từng syscall.
static CORE_VALUE: [AtomicU64; CORE_SYSCALL_RECORDS] =
    [const { AtomicU64::new(0) }; CORE_SYSCALL_RECORDS];

/// Báo cáo của `core_bridge::init` (in lại ở `finish`).
static REPORT_ADDR: AtomicU64 = AtomicU64::new(0);
static REPORT_BYTES: AtomicU64 = AtomicU64::new(0);
static REPORT_PIN: AtomicU64 = AtomicU64::new(0);
static REPORT_HARTS: AtomicU64 = AtomicU64::new(0);
static REPORT_HEALTHY: AtomicBool = AtomicBool::new(false);
/// PA frame IPC buffer.
static BUF_PA: AtomicU64 = AtomicU64::new(0);
/// `CR3` gốc của ảnh (khôi phục sau pha core).
static BOOT_ROOT: AtomicU64 = AtomicU64::new(0);
/// `CR3` của `VSpace` pha core.
static CORE_ROOT: AtomicU64 = AtomicU64::new(0);

/// TCB hiện hành của đường syscall (`None` ⇔ rig chưa dựng).
pub fn current_tcb() -> Option<ObjId> {
    let tcb = CURRENT_TCB.load(Ordering::Acquire);
    (tcb != NO_TCB).then_some(tcb)
}

/// `true` ⇔ đang ở pha core (chương trình ring 3 thứ hai).
pub fn in_core_phase() -> bool {
    CORE_PHASE.load(Ordering::Acquire)
}

/// `true` ⇔ mọi thứ đã dựng xong (dùng cho TAP 38).
pub fn ready() -> bool {
    MAIN_TCB.load(Ordering::Acquire) != NO_TCB && REPORT_HEALTHY.load(Ordering::Acquire)
}

// ---------------------------------------------------------------------------------------------
// Dựng rig — gọi trong `kmain` **trước** lần `iretq` đầu tiên (syscall của cả hai lượt ring 3
// đi qua `vifast-core`, nên `Machine` + một TCB `Running` phải có trước lượt 1)
// ---------------------------------------------------------------------------------------------

/// Cấp một frame từ arena bảng trang; hết frame ⇒ in + halt (fail-loud, không đoán).
fn alloc(console: &mut Console, arena: &mut BootArena, what: &str) -> u64 {
    match arena.alloc_frame() {
        Some(pa) => pa,
        None => {
            let _ = writeln!(
                console,
                "PANIC: coretest: arena hết frame khi cấp '{what}' (đã dùng {} / {})",
                arena.used(),
                arena.frames(),
            );
            asm::halt(3)
        }
    }
}

/// Dựng `Machine`, CSpace và TCB của **lượt ring 3 đầu** (DebugPutStr/Exit) + toàn bộ object của
/// pha core; công bố biên ảnh cho `core_bridge`; điền sẵn IPC buffer.
///
/// Trả `false` ⇔ bất kỳ bước nào hỏng (ảnh in `PANIC` — fail-loud, không chạy tiếp với model nửa
/// vời). Mọi lỗi core đều có mã; rig **không** nuốt lỗi.
pub fn prepare(console: &mut Console) -> bool {
    // ---- 0) Biên vùng ảnh — x86_64 chạy `VA == PA` (identity map của `_start`).
    let text_start = core::ptr::addr_of!(__text_start) as u64;
    let image_end = core::ptr::addr_of!(__kernel_end) as u64;
    core_bridge::set_image_bounds(text_start, image_end);
    let image_bytes = image_end.saturating_sub(text_start);

    // ---- 1) Untyped boot: 4 frame 4 KiB liền nhau từ arena (nhãn PA — D2-83). Arena dùng **cùng**
    // bộ đếm chia sẻ với `umode::run`/`coretest` nên không bao giờ cấp trùng frame.
    let Some(mut arena) = BootArena::new(
        crate::bootpt::arena_base(),
        crate::bootpt::arena_frames(),
        crate::bootpt::arena_next(),
    ) else {
        let _ = writeln!(console, "# core fail: stage=arena (arena không hợp lệ)");
        return false;
    };
    let mut untyped_base = 0u64;
    for index in 0..4 {
        let pa = alloc(console, &mut arena, "untyped");
        if index == 0 {
            untyped_base = pa;
        }
    }

    // ---- 2) Trang IPC buffer: một frame, zero rồi điền header + vùng cấu hình + MR4..MR7.
    let buf_pa = alloc(console, &mut arena, "ipc-buffer");
    for offset in (0..0x1000).step_by(8) {
        asm::write_u64(buf_pa + offset, 0);
    }
    BUF_PA.store(buf_pa, Ordering::Release);
    fill_user_buffer(buf_pa, PRIORITY_OLD, SLOT_VSPACE as u64, 0);

    // ---- 3) Model core + rig (một `with_machine`, không lồng khoá; không in bên trong).
    let harts = 1usize; // lúc `prepare` chỉ BSP online (SMP boot ở TAP 18..21, sau pha này).
    let report = core_bridge::init(untyped_base, 4 * 0x1000, harts);
    REPORT_ADDR.store(report.addr, Ordering::Release);
    REPORT_BYTES.store(report.static_bytes as u64, Ordering::Release);
    REPORT_PIN.store(report.pin as u64, Ordering::Release);
    REPORT_HARTS.store(report.harts as u64, Ordering::Release);
    REPORT_HEALTHY.store(report.healthy(), Ordering::Release);
    if !report.healthy() {
        let _ = writeln!(
            console,
            "# core fail: stage=report addr=0x{:x} size={} pin={} in_image={} caps={} bytes={} all_bytes={} image=[0x{text_start:x},0x{image_end:x}) bytes={image_bytes}",
            report.addr,
            report.static_bytes,
            report.pin,
            u8::from(report.in_image),
            u8::from(report.check_caps),
            u8::from(report.check_bytes),
            u8::from(report.check_all_bytes),
        );
        return false;
    }

    let built = core_bridge::with_machine(|m| -> Result<(), KError> {
        UNTYPED.store(report.untyped, Ordering::Release);
        let root = m.add_cnode(ROOT_SLOTS);
        ROOT.store(root, Ordering::Release);
        let uc = m.root_cap(report.untyped);
        m.put_cap(root, SLOT_UNTYPED, uc)?;
        let us = SlotRef::new(root, SLOT_UNTYPED);

        // TCB lượt 1 (DebugPutStr/Exit) — chỉ cần CSpace + `Running`.
        let turn1_cap = m.retype(us, ObjType::Tcb, 0, 0)?;
        m.put_cap(root, SLOT_TCB, turn1_cap)?;
        let turn1 = m.cap_obj(turn1_cap)?;
        // TCB pha core — tác vụ gọi invocation trên **chính nó** (Configure/Suspend).
        let core_cap = m.retype(us, ObjType::Tcb, 0, 0)?;
        m.put_cap(root, SLOT_CORE, core_cap)?;
        let core = m.cap_obj(core_cap)?;
        MAIN_TCB.store(core, Ordering::Release);
        // TCB server của ca (b) — `Recv` chặn trên EP.
        let srv_cap = m.retype(us, ObjType::Tcb, 0, 0)?;
        m.put_cap(root, SLOT_SERVER, srv_cap)?;
        let server = m.cap_obj(srv_cap)?;
        SERVER.store(server, Ordering::Release);
        // TCB người gửi của ca (d) — bị chặn `Send` trên EP2 trước khi ring 3 `Recv`.
        let snd_cap = m.retype(us, ObjType::Tcb, 0, 0)?;
        m.put_cap(root, SLOT_SENDER, snd_cap)?;
        let sender = m.cap_obj(snd_cap)?;
        SENDER.store(sender, Ordering::Release);
        // Hai Endpoint.
        let ep_cap = m.retype(us, ObjType::Endpoint, 0, 0)?;
        m.put_cap(root, SLOT_EP, ep_cap)?;
        let ep2_cap = m.retype(us, ObjType::Endpoint, 0, 0)?;
        m.put_cap(root, SLOT_EP2, ep2_cap)?;
        // Cap `CNode` của **chính** CSpace gốc.
        let cspace_cap = m.root_cap(root);
        m.put_cap(root, SLOT_CSPACE, cspace_cap)?;
        // Frame IPC buffer: object **đại diện** (D2-83 — object layer sở hữu Frame thật).
        let frame_obj = m.add_foreign_object(ObjType::Frame, buf_pa)?;
        let frame_cap = m.root_cap(frame_obj);
        m.put_cap(root, SLOT_FRAME, frame_cap)?;
        // VSpace **đại diện** cho ca `vspace?` — arch từ chối giữ ⇒ `Failed(Busy)`.
        let vspace_obj = m.add_foreign_object(ObjType::VSpace, 0x340c_5653)?;
        let vspace_cap = m.root_cap(vspace_obj);
        m.put_cap(root, SLOT_VSPACE, vspace_cap)?;

        // Lượt 1: CSpace + `Running` (syscall 12/14 không dùng cap).
        m.tcb_configure(
            SlotRef::new(root, SLOT_TCB),
            Some(SlotRef::new(root, SLOT_CSPACE)),
        )?;
        m.start_tcb(turn1)?;
        m.transition_tcb(turn1, NonReadyTarget::Running { hart: 0 })?;
        CURRENT_TCB.store(turn1, Ordering::Release);
        // Pha core: CSpace + IPC buffer + priority 4 (chạy ở `start_core_phase`).
        m.tcb_configure_with_ipc_buffer(
            SlotRef::new(root, SLOT_CORE),
            Some(SlotRef::new(root, SLOT_CSPACE)),
            Some((SlotRef::new(root, SLOT_FRAME), IPC_BUF_VA)),
        )?;
        m.tcb_set_priority(SlotRef::new(root, SLOT_CORE), PRIORITY_OLD, u8::MAX)?;
        // Server + sender: cùng CSpace; server `Recv`-chặn, sender `Send`-chặn ở `start_core_phase`.
        for tcb in [server, sender] {
            m.tcb_configure(
                SlotRef::new(root, slot_of(root, tcb)?),
                Some(SlotRef::new(root, SLOT_CSPACE)),
            )?;
            m.start_tcb(tcb)?;
            m.transition_tcb(tcb, NonReadyTarget::Running { hart: 0 })?;
        }
        Ok(())
    });
    if built.is_err() {
        let _ = writeln!(
            console,
            "PANIC: coretest: dựng rig lỗi (xem dòng # core fail phía trên)"
        );
        return false;
    }
    true
}

/// Ô chứa cap của `tcb` trong CSpace gốc (chỉ dùng cho hai TCB rig vừa đặt).
fn slot_of(_root: ObjId, tcb: ObjId) -> Result<usize, KError> {
    // Hai TCB rig nằm ở ô cố định; ánh xạ theo `ObjId` để không phụ thuộc thứ tự retype.
    if tcb == SERVER.load(Ordering::Acquire) {
        Ok(SLOT_SERVER)
    } else if tcb == SENDER.load(Ordering::Acquire) {
        Ok(SLOT_SENDER)
    } else {
        Err(KError::InvalidCapability)
    }
}

/// Điền **header IPC buffer** (magic + window + vùng extra caps + vùng `Tcb.Configure`) và
/// MR4..MR7 — ghi bằng PA (identity map) **trước** khi `VSpace` của pha core map nó.
fn fill_user_buffer(pa: u64, priority: u8, vspace_cptr: u64, entry: u64) {
    let base = pa;
    let set = |offset: usize, value: u64| asm::write_u64(base + offset as u64, value);
    // Header 10 word: magic, info, receive window 4 CPTR, 4 CPTR nguồn cap.
    set(0, IPC_HEADER_MAGIC);
    set(8, 0);
    // Window 4 ô **rỗng** của CSpace 16 ô (12..15 — không dùng bởi rig).
    for (index, slot) in [12u64, 13, 14, 15].iter().enumerate() {
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

/// Ghi lại **một** ô của vùng `Tcb.Configure` (dùng giữa hai lần vào ring 3 của pha core).
fn set_configure_word(pa: u64, index: usize, value: u64) {
    asm::write_u64(
        pa + IPC_BUFFER_CONFIGURE_OFFSET as u64 + (index as u64) * 8,
        value,
    );
}

// ---------------------------------------------------------------------------------------------
// Pha core — chương trình ring 3 thứ hai
// ---------------------------------------------------------------------------------------------

/// Bắt đầu pha core: chặn server `Recv` + người gửi `Send` (nguồn message ca (d)), đổi TCB hiện
/// hành, dựng `VSpace` **riêng** map mã + stack + IPC buffer, rồi `iretq` xuống CPL3.
///
/// **Không trả về** — đường `Exit` của pha core quay lại handler `int 0x80` (kernel tiếp quản).
pub fn start_core_phase(console: &mut Console, boot_root: u64) -> ! {
    BOOT_ROOT.store(boot_root, Ordering::Release);
    let tcb: ObjId = MAIN_TCB.load(Ordering::Acquire);
    let entry_off = __user_core_entry as *const () as u64 - code_start();
    let after_off = __user_core_after_suspend as *const () as u64 - code_start();
    if entry_off >= 0x1000 || after_off >= 0x1000 {
        let _ = writeln!(
            console,
            "PANIC: coretest: entry ngoài trang .user (entry=0x{entry_off:x} after=0x{after_off:x})"
        );
        asm::halt(3)
    }
    let entry = VSPACE_CODE_VA + entry_off;
    let after = VSPACE_CODE_VA + after_off;
    // Điền vùng `Tcb.Configure` cho pha core: `entry?` = điểm vào sau `Suspend` (core giao lại
    // qua `Tcb.Resume`) và `priority?` = 9 — chương trình ring 3 **yêu cầu** trị này qua buffer,
    // nên kernel không thể tự đặt 9 rồi tự khen (bằng chứng "đổi ưu tiên từ ring 3").
    let buf_pa = BUF_PA.load(Ordering::Acquire);
    set_configure_word(buf_pa, 2, after);
    set_configure_word(buf_pa, 3, u64::from(PRIORITY_SET));

    let Some(arena) = BootArena::new(
        crate::bootpt::arena_base(),
        crate::bootpt::arena_frames(),
        crate::bootpt::arena_next(),
    ) else {
        let _ = writeln!(console, "PANIC: coretest: arena không hợp lệ");
        asm::halt(3)
    };
    let mut vs = match VSpace::create(3, arena) {
        Ok(vs) => vs,
        Err(err) => {
            let _ = writeln!(console, "PANIC: coretest: tạo VSpace lỗi {err:?}");
            asm::halt(3)
        }
    };
    let code_pa = code_start();
    let stack_pa = core::ptr::addr_of!(__user_stack) as u64;
    let buf_pa = BUF_PA.load(Ordering::Acquire);
    for (va, pa, perms, what) in [
        (VSPACE_CODE_VA, code_pa, Perms::USER_RX, "code"),
        (VSPACE_STACK_VA, stack_pa, Perms::USER_RW, "stack"),
        (IPC_BUF_VA, buf_pa, Perms::USER_RW, "ipc-buffer"),
    ] {
        if let Err(err) = vs.map(va, pa, perms) {
            let _ = writeln!(
                console,
                "PANIC: coretest: map {what} va=0x{va:x} pa=0x{pa:x} lỗi {err:?}"
            );
            asm::halt(3)
        }
    }
    let root = vs.root();
    CORE_ROOT.store(root, Ordering::Release);
    syscall_env::set_ipc_buffer_va(Some(IPC_BUF_VA));

    // Chặn server `Recv` **trước** khi ring 3 gửi (ca (b) giao ngay, không cần Reply) và chặn
    // người gửi `Send` trên EP2 **trước** khi ring 3 `Recv` (ca (d) nhận được message ⇒
    // `dispatch` trả `MessageInfo` ở `r13`).
    let ready = core_bridge::with_machine(|m| -> Result<(), KError> {
        let root = ROOT.load(Ordering::Acquire);
        if matches!(m.tcb_state(tcb), Ok(TcbState::Inactive)) {
            m.start_tcb(tcb)?;
        }
        m.transition_tcb(tcb, NonReadyTarget::Running { hart: 0 })?;
        CURRENT_TCB.store(tcb, Ordering::Release);
        m.recv(
            SERVER.load(Ordering::Acquire),
            SlotRef::new(root, SLOT_EP),
            None,
        )?;
        let words = [
            MSG_RECV0,
            MSG_RECV1,
            MSG_RECV2,
            MSG_RECV3,
            0x3410_0004,
            0x3410_0005,
            0x3410_0006,
            0x3410_0007,
        ];
        let msg = Message::with_cap_ids(MSG_RECV_LABEL, &words, &[])?;
        m.send(
            SENDER.load(Ordering::Acquire),
            SlotRef::new(root, SLOT_EP2),
            msg,
        )?;
        Ok(())
    });
    if ready.is_err() {
        let _ = writeln!(console, "PANIC: coretest: rig IPC chặn lỗi");
        asm::halt(3)
    }
    CORE_PHASE.store(true, Ordering::Release);

    // Đổi sang `VSpace` của pha core **trước** `iretq`; stack kernel của đường syscall vẫn là
    // `__syscall_stack_top` (đã đặt ở lượt 1; `TSS` per-hart).
    let syscall_stack_top = core::ptr::addr_of!(__syscall_stack_top) as u64;
    tss::set_rsp0(0, syscall_stack_top);
    vs.activate();
    let _ = writeln!(
        console,
        "# core phase: entry=0x{entry:x} after=0x{after:x} buf_va=0x{IPC_BUF_VA:x} buf_pa=0x{buf_pa:x} root=0x{root:x} boot_root=0x{boot_root:x} tcb={tcb}"
    );
    asm::enter_user(entry, VSPACE_STACK_TOP, USER_RFLAGS)
}

/// Gọi từ handler khi `dispatch` trả `Switched` (điểm an toàn `Suspend(Defer)`): đo trạng thái,
/// `Tcb.Resume` (method `TCB_RESUME`) rồi trả **điểm vào** để handler ghi vào frame `RIP`.
///
/// `None` ⇔ `Resume`/hook hỏng — handler halt (ĐỎ, không chạy tiếp vào rác).
pub fn resume_after_switch(console: &mut Console, state: TcbState) -> Option<u64> {
    let tcb: ObjId = current_tcb()?;
    SWITCHED.fetch_add(1, Ordering::Relaxed);
    SUSPEND_STATE.store(state_code(state), Ordering::Release);
    let info = msg_info(vifast_core::invoke::TCB_RESUME, 0);
    let result = core_bridge::with_machine(|m| {
        invoke::dispatch(
            m,
            tcb,
            SlotRef::new(ROOT.load(Ordering::Acquire), SLOT_CORE),
            info,
            &[0; 4],
            &mut X86_64Env,
        )
    });
    let entry = match result {
        Ok((0, entry)) => entry,
        Ok((error, value)) => {
            let _ = writeln!(
                console,
                "PANIC: coretest: Tcb.Resume lỗi error={error} value={value} state=0x{:x}",
                state_code(state)
            );
            return None;
        }
        Err(error) => {
            let _ = writeln!(
                console,
                "PANIC: coretest: Tcb.Resume Err {error:?} state=0x{:x}",
                state_code(state)
            );
            return None;
        }
    };
    RESUME_ENTRY.store(entry, Ordering::Release);
    // Arch **tiêu thụ** `entry` do `Tcb.Resume` giao qua hook `SyscallEnv::tcb_resume`.
    let pc = syscall_env::take_resume(tcb)?;
    RESUME_OK.store(pc == entry, Ordering::Release);
    let _ = writeln!(
        console,
        "# core resume: tcb={tcb} state_after_switch=0x{:x} entry=0x{entry:x} match={}",
        state_code(state),
        u8::from(pc == entry),
    );
    Some(entry)
}

/// `TcbState` → số ổn định để ghi log.
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

/// Tên trạng thái cho log (`# core suspend:`) từ giá trị `TcbState`.
fn state_name_of(state: TcbState) -> &'static str {
    state_name(state_code(state))
}

/// Tên trạng thái theo mã ổn định.
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

/// Ghi kết quả **một** syscall của pha core (handler gọi) — chỉ số = thứ tự gọi, không do ring 3
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

/// Đọc `CORE_ERROR[i]` (`NO_U64` ⇒ in ra số lớn — luôn khác mọi mã lỗi thật ⇒ TAP đỏ).
fn field(array: &[AtomicU64; CORE_SYSCALL_RECORDS], index: usize) -> u64 {
    array[index].load(Ordering::Acquire)
}

/// Khôi phục `CR3` gốc rồi in bằng chứng dạng dòng `# core:`.
pub fn report(console: &mut Console) {
    let boot_root = BOOT_ROOT.load(Ordering::Acquire);
    if boot_root != 0 {
        asm::write_cr3(boot_root);
    }
    let (objects, caps) = core_bridge::with_machine(|m| (m.object_count(), m.live_cap_count()));
    let _ = writeln!(
        console,
        "# core: addr=0x{:x} size={} pin={} in_image={} harts={} objects={} caps={} healthy={}",
        REPORT_ADDR.load(Ordering::Acquire),
        REPORT_BYTES.load(Ordering::Acquire),
        REPORT_PIN.load(Ordering::Acquire),
        u8::from(core_bridge::in_image_region(
            REPORT_ADDR.load(Ordering::Acquire),
            REPORT_BYTES.load(Ordering::Acquire) as usize
        )),
        REPORT_HARTS.load(Ordering::Acquire),
        objects,
        caps,
        u8::from(REPORT_HEALTHY.load(Ordering::Acquire)),
    );
    let (mr, extra, configure) = vifast_plat_x86_64::syscall_env::LAYOUT_EVIDENCE;
    let _ = writeln!(
        console,
        "# core buffer: va=0x{IPC_BUF_VA:x} mr_offset={mr} extra_offset={extra} configure_offset={configure} harts={}",
        core_bridge::harts(),
    );
    let _ = writeln!(
        console,
        "# core roots: boot=0x{boot_root:x} core=0x{:x}",
        CORE_ROOT.load(Ordering::Acquire),
    );
}

/// **Điểm hội tụ** của pha core: đo lại toàn bộ bằng chứng, phát TAP 38..43 rồi **trả về** cho
/// handler gọi tiếp `net::run` (TAP 15.. — chuỗi boot không đổi thứ tự).
///
/// `check` **halt ngay** khi một ca đỏ ⇒ không in marker (bất biến của runner). Mọi kết luận lấy
/// từ `Machine` **và** từ mã thoát của ring 3 — hai nguồn độc lập.
pub fn finish(console: &mut Console) {
    // ---- Khoá/ngắt (TAP 43): cần `RFLAGS.IF` **đang mở** để phép đo "đã khôi phục" có nghĩa.
    // `cli` lại sau phép đo để các pha sau khởi đầu đúng trạng thái mà chúng chờ (chúng tự `sti`).
    asm::sti();
    let irq_before = irq_enabled();
    let mut irq_inside = true;
    core_bridge::with_machine(|_| {
        irq_inside = irq_enabled();
    });
    let irq_after = irq_enabled();
    asm::cli();
    let lock = core_bridge::lock_stats();
    let lock_ok = irq_before && !irq_inside && irq_after && lock.ok();

    // ---- Đo trạng thái cuối của `Machine`.
    let root = ROOT.load(Ordering::Acquire);
    let facts = core_bridge::with_machine(|m| Facts {
        priority: m
            .tcb_info(SlotRef::new(ROOT.load(Ordering::Acquire), SLOT_CORE))
            .map(|i| i.priority)
            .unwrap_or(0),
        state: m.tcb_state(MAIN_TCB.load(Ordering::Acquire)).ok(),
        vspace: m
            .tcb_vspace(MAIN_TCB.load(Ordering::Acquire))
            .unwrap_or(NO_TCB),
        server_state: m.tcb_state(SERVER.load(Ordering::Acquire)).ok(),
        area: m.tcb_ipc_area(SERVER.load(Ordering::Acquire)).ok(),
        recv_area: m.tcb_ipc_area(MAIN_TCB.load(Ordering::Acquire)).ok(),
        caps_ok: m.check_caps().is_ok(),
        bytes_ok: m.check_bytes(UNTYPED.load(Ordering::Acquire)).is_ok(),
        all_bytes_ok: m.check_all_bytes().is_ok(),
        ipc_ok: m.check_ipc().is_ok(),
        objects: m.object_count(),
        caps: m.live_cap_count(),
        _root: root,
    });
    let stats = invoke::stats();
    let exit_code = EXIT_CODE.load(Ordering::Acquire);
    let mask = exit_code;

    // ---- Bằng chứng dạng dòng `#` (máy kiểm được, không phải trang trí).
    report(console);
    let _ = writeln!(
        console,
        "# core configure: priority_old={PRIORITY_OLD} priority_now={} err_cfg={} err_vspace={} err_entry={} vspace={} configure={}",
        facts.priority,
        field(&CORE_ERROR, 0),
        field(&CORE_ERROR, 1),
        field(&CORE_ERROR, 4),
        facts.vspace,
        stats.configure,
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
        field(&CORE_ERROR, 2),
        msg_len,
        facts.area.as_ref().map(|a| a.regs[0]).unwrap_or(0),
        facts.area.as_ref().map(|a| a.regs[3]).unwrap_or(0),
        msg_mr4,
        msg_mr7,
        facts.server_state.map(state_name_of).unwrap_or("none"),
    );
    let (recv_info, recv_mr3) = match &facts.recv_area {
        Some(area) => match area.info {
            Some(info) => (info.pack(), area.regs[3]),
            None => (0, 0),
        },
        None => (0, 0),
    };
    let _ = writeln!(
        console,
        "# core 7c-r13: recv_err={} exit=0x{exit_code:x} info_seen_expected=0x{INFO_RECV:x} mr3_expected=0x{MSG_RECV3:x} area_info=0x{recv_info:x} area_mr3=0x{recv_mr3:x}",
        field(&CORE_ERROR, 3),
    );
    let _ = writeln!(
        console,
        "# core suspend: switched={} state={} resume_entry=0x{:x} resume_ok={} exit=0x{:x} deferred={} resume_stat={}",
        SWITCHED.load(Ordering::Acquire),
        state_name(SUSPEND_STATE.load(Ordering::Acquire)),
        RESUME_ENTRY.load(Ordering::Acquire),
        u8::from(RESUME_OK.load(Ordering::Acquire)),
        exit_code,
        stats.deferred,
        stats.resume,
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

    // ---- TAP 38..43 — mỗi ca một điều kiện **đo được**, không "ok" mặc định.
    let report_bytes = REPORT_BYTES.load(Ordering::Acquire) as usize;
    check(
        console,
        38,
        "core: Machine linked in image, pin + invariants",
        ready()
            && core_bridge::in_image_region(REPORT_ADDR.load(Ordering::Acquire), report_bytes)
            && report_bytes <= REPORT_PIN.load(Ordering::Acquire) as usize,
    );
    check(
        console,
        39,
        "core: Tcb.Configure priority + vspace rollback",
        field(&CORE_ERROR, 0) == 0
            && field(&CORE_ERROR, 1) == u64::from(KError::Busy.id())
            && facts.priority == PRIORITY_SET
            && facts.vspace == 0
            && stats.configure >= 2,
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
    check(
        console,
        40,
        "ipc: ring3 send 8 MR -> endpoint recv (MR4..MR7 buffer)",
        field(&CORE_ERROR, 2) == 0
            && mr_match
            && matches!(facts.server_state, Some(TcbState::Ready)),
    );
    // `7C-r13`: info trả **phải** ở `r13` và `r12` **vẫn** là MR3 (đột biến `mut-info-r12` ghi
    // info vào `r12` ⇒ bit 0/1 của mask bật ⇒ TAP này ĐỎ). Hai nguồn: mask do ring 3 tự so, và
    // vùng nhận của TCB do **kernel** đọc.
    check(
        console,
        41,
        "core: 7c-r13 MessageInfo from r13 (r12 keeps MR3)",
        field(&CORE_ERROR, 3) == 0
            && mask != NO_U64
            && mask & 0b1011 == 0
            && recv_info == INFO_RECV
            && recv_mr3 == MSG_RECV3,
    );
    check(
        console,
        42,
        "core: Tcb.Suspend(Defer) safe point + Resume entry",
        SWITCHED.load(Ordering::Acquire) == 1
            && SUSPEND_STATE.load(Ordering::Acquire) == state_code(TcbState::Inactive)
            && SUSPEND_STATE.load(Ordering::Acquire) != NO_U64
            && RESUME_OK.load(Ordering::Acquire)
            && RESUME_ENTRY.load(Ordering::Acquire) != 0
            && field(&CORE_ERROR, 4) == 0
            && mask != NO_U64
            && mask & 4 != 0
            && stats.deferred == 1,
    );
    check(
        console,
        43,
        "lock: RFLAGS.IF masked in critical section, restored after",
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
            facts.state.map(state_name_of).unwrap_or("none"),
        );
        asm::halt(5)
    }
    let _ = writeln!(console, "# core: done");
}

/// Ảnh chụp trạng thái cuối của `Machine` (một lần khoá, đọc đủ cho mọi TAP).
struct Facts {
    priority: u8,
    state: Option<TcbState>,
    vspace: u32,
    server_state: Option<TcbState>,
    area: Option<vifast_core::TcbIpcArea>,
    recv_area: Option<vifast_core::TcbIpcArea>,
    caps_ok: bool,
    bytes_ok: bool,
    all_bytes_ok: bool,
    ipc_ok: bool,
    objects: usize,
    caps: usize,
    _root: ObjId,
}

// ---------------------------------------------------------------------------------------------
// Chương trình ring 3 của pha core — trong **cùng** section `.user` (một trang 4 KiB) như
// `__user_entry`, nên `VSpace` của pha core map **cùng PA** và mọi địa chỉ là PC-relative.
// ---------------------------------------------------------------------------------------------

/// Đầu section `.user` (PA trong ảnh — `VA == PA`).
fn code_start() -> u64 {
    core::ptr::addr_of!(__user_start) as u64
}

extern "C" {
    /// Đầu `.text` — biên dưới vùng ảnh (identity map nên VA == PA).
    static __text_start: u8;
    /// Cuối vùng ảnh (`linker.ld`) — biên trên vùng ảnh.
    static __kernel_end: u8;
    /// Trang stack user của ring 3 (`.bootstacks`, identity map).
    static __user_stack: u8;
    /// Đỉnh stack kernel riêng cho đường vào từ ring 3 (`TSS.RSP0`).
    static __syscall_stack_top: u8;
    static __user_start: u8;
    /// Điểm vào chương trình ring 3 của pha core (K5: `.user` — xem `bootasm.rs`).
    fn __user_core_entry();
    /// Điểm vào **sau** `Tcb.Suspend(Defer)` + `Tcb.Resume` (core giao qua `entry?`).
    fn __user_core_after_suspend();
}

global_asm!(
    ".section .user, \"ax\"",
    ".globl __user_core_entry",
    "__user_core_entry:",
    // `r14` = **mặt nạ lỗi** do chính chương trình ring 3 tính (bit 0 = `r13` sai, bit 1 =
    // `r12` sai, bit 2 = đã chạy tiếp sau `Resume`, bit 3 = một syscall trả lỗi).
    "xor r14, r14",
    // (a) `Tcb.Configure` đổi **ưu tiên**: Call(cap TCB core, info{label=Configure,len=3},
    //     MR0 = CPTR Frame, MR1 = VA buffer, MR2 = mask priority). Giá trị ưu tiên ở IPC buffer.
    "mov rax, {sys_call}",
    "mov rdi, {slot_core}",
    "mov rdx, {info_cfg3}",
    "mov r10, {slot_frame}",
    "mov r8, {buf_va}",
    "mov r9, {mask_prio}",
    "int 0x80",
    "test rax, rax",
    "jz 1f",
    "or r14, 8",
    "1:",
    // (a2) `Tcb.Configure(vspace?)` trên cap `VSpace` **đại diện** ⇒ arch `Failed(Busy)` ⇒ core
    //      rollback + trả 14. Giá trị `vspace` ở IPC buffer (word 616).
    "mov rax, {sys_call}",
    "mov rdi, {slot_core}",
    "mov rdx, {info_cfg3}",
    "mov r10, {slot_frame}",
    "mov r8, {buf_va}",
    "mov r9, {mask_vspace}",
    "int 0x80",
    "cmp rax, 14",
    "je 2f",
    "or r14, 8",
    "2:",
    // (b) `Send` message 8 MR trên Endpoint (server đã Recv-blocked): MR0..MR3 thanh ghi,
    //     MR4..MR7 trong IPC buffer (offset 80).
    "mov rax, {sys_send}",
    "mov rdi, {slot_ep}",
    "mov rdx, {info_send}",
    "mov r10, {mr0}",
    "mov r8, {mr1}",
    "mov r9, {mr2}",
    "mov r12, {mr3}",
    "int 0x80",
    "test rax, rax",
    "jz 3f",
    "or r14, 8",
    "3:",
    // (d) `Recv` trên EP2: người gửi đã bị chặn **trước** khi vào ring 3, nên hàng đợi có message
    //     ⇒ `dispatch` trả `MessageInfo` ở **`r13`** (`7C-r13`) + MR3 ở `r12`.
    "mov rax, {sys_recv}",
    "mov rdi, {slot_ep2}",
    "mov rsi, -1", // REPLY_NONE — không có cap Reply
    "int 0x80",
    "test rax, rax",
    "jz 4f",
    "or r14, 8",
    "4:",
    // (d1) `r13` **phải** là `MessageInfo` đã đóng gói của message vừa nhận. So bằng thanh ghi
    //      (`cmp r64, imm64` không mã hoá được nên nạp hằng vào `r15`).
    "mov r15, {info_recv}",
    "cmp r13, r15",
    "je 5f",
    "or r14, 1",
    "5:",
    // (d2) `r12` **phải** vẫn là MR3 của message — nếu kernel đọc/ghi MessageBox ở `r12`
    //      (đột biến `mut-info-r12`) thì `r12` = `MessageInfo` ≠ MR3 ⇒ bit 1.
    "mov r15, {msg_recv3}",
    "cmp r12, r15",
    "je 6f",
    "or r14, 2",
    "6:",
    // (c1) `Tcb.Configure(entry?)` = `__user_core_after_suspend` (để `Tcb.Resume` có điểm vào).
    "mov rax, {sys_call}",
    "mov rdi, {slot_core}",
    "mov rdx, {info_cfg3}",
    "mov r10, {slot_frame}",
    "mov r8, {buf_va}",
    "mov r9, {mask_entry}",
    "int 0x80",
    "test rax, rax",
    "jz 7f",
    "or r14, 8",
    "7:",
    // (c2) `Tcb.Suspend(self, Defer)` — `dispatch` trả `Switched` ở **điểm an toàn** ⇒ kernel
    //      tiếp quản, `Resume` rồi quay lại đúng `__user_core_after_suspend`.
    "mov rax, {sys_call}",
    "mov rdi, {slot_core}",
    "mov rdx, {info_suspend}",
    "mov r10, {defer}",
    "int 0x80",
    // Defer **không** được trả về đây (nếu trả ⇒ SAI): Exit(99) để kernel thấy mã thoát.
    "mov rax, {sys_exit}",
    "mov rdi, 99",
    "int 0x80",
    "8:",
    "jmp 8b",
    // `Tcb.Configure(entry?)` đòi `entry % TCB_ENTRY_ALIGN (2) == 0`; nhãn nằm sau một loạt
    // lệnh dài ngắn khác nhau nên phải căn 2 B tường minh (`nop` đệm).
    ".balign 2, 0x90",
    ".globl __user_core_after_suspend",
    "__user_core_after_suspend:",
    // Đã chạy tiếp đúng điểm vào do `Tcb.Resume` trả ⇒ bit 2; rồi Exit(mask).
    "or r14, 4",
    "mov rax, {sys_exit}",
    "mov rdi, r14",
    "int 0x80",
    "9:",
    "jmp 9b",
    sys_call = const vifast_abi::SYS_CALL,
    sys_send = const vifast_abi::SYS_SEND,
    sys_recv = const vifast_abi::SYS_RECV,
    sys_exit = const vifast_abi::SYS_EXIT,
    slot_core = const SLOT_CORE as u64,
    slot_ep = const SLOT_EP as u64,
    slot_ep2 = const SLOT_EP2 as u64,
    slot_frame = const SLOT_FRAME as u64,
    buf_va = const IPC_BUF_VA,
    mr0 = const MSG_MR0,
    mr1 = const MSG_MR1,
    mr2 = const MSG_MR2,
    mr3 = const MSG_MR3,
    msg_recv3 = const MSG_RECV3,
    info_cfg3 = const INFO_CFG3,
    info_send = const INFO_SEND,
    info_recv = const INFO_RECV,
    info_suspend = const INFO_SUSPEND,
    mask_prio = const MASK_PRIORITY,
    mask_vspace = const MASK_VSPACE,
    mask_entry = const MASK_ENTRY,
    defer = const SUSPEND_MODE_DEFER,
);

const _: () = assert!(ROOT_SLOTS <= 1 << 16);
const _: () = assert!(vifast_core::ipc::WORDS_IN_REGISTERS == 4);
const _: () = assert!(TCB_STATE_COUNT == 11);
/// `WORDS_IN_IPC_BUFFER` được import để chốt biên (message 8 MR ⇒ 4 word buffer).
const _: () = assert!(WORDS_IN_IPC_BUFFER >= 4);
/// Import thuộc về chữ ký `SyscallEnv`/`MessageInfo` — dùng để kiểm biên dịch (không chết).
const _: fn() = || {
    let _ = <X86_64Env as SyscallEnv>::features;
    let _ = SyscallOutcome::Exit { code: 0 };
    let _ = umode::VSPACE_STACK_TOP;
};
