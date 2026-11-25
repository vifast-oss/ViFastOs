// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-280** — ảnh test **IPC từ U-mode** riscv64: client U-mode `Call` **chặn** tới **server
//! U-mode**, server `Recv` + `Reply`, client được **resume** với message trả lời.
//!
//! **KT-282** — mở rộng cùng ảnh: **MR4..MR63 qua IPC buffer** (nợ K3.5 của KT-280 §7.3).
//!
//! **KT-290** — trả nợ KT-282 §6.3: **`Tcb.Configure` qua invocation từ U-mode**. Trước KT-290
//! rig gọi thẳng `Machine::tcb_configure_with_ipc_buffer` để bind IPC buffer của client; nay
//! **chính chương trình U-mode** gọi `Call` trên cap **TCB** với `label = TCB_CONFIGURE` (0, hằng
//! sinh từ `spec/abi.toml`) và `length = 2` (`MR0` = CPTR cap `Frame`, `MR1` = VA) ⇒ kernel bind
//! buffer rồi trả `(error, value) = (0, 0)`. Rig **không** bind hộ client nữa ⇒ vòng 8 MR/64 MR
//! chỉ xanh nếu invocation chạy thật. TAP 4 đo đường đó; ba ca âm (cap không phải TCB, method id
//! sai, buffer ngoài frame) nằm trong cùng boot.
//!
//! # Vì sao tách ảnh
//!
//! D2-109/D2-111: mỗi ảnh test một ngân sách (262.144 B `.data+.bss`), thêm **đường mới** ⇒ tách
//! ảnh, không nới trần. Ảnh này tách khỏi `ktest-user`/`ktest-proc` để TAP/`.text` của chúng
//! **không** đổi (hồi quy matrix cũ vẫn so đúng ảnh cũ).
//!
//! # Vì sao cần (khảo sát KT-280 §1 của `JOURNAL/kt280-ipc.md`)
//!
//! K3 đã có IPC **trong core** (TAP 9 của `ktest-user` gọi `Call/Recv/Send` **từ kernel**) và
//! U-mode đã có từ KT-53, nhưng **chưa** có đường "tác vụ U-mode chặn trong IPC rồi được resume":
//! mọi `call()` của chương trình U-mode đều nhắm cap **không phải Endpoint** (invocation ⇒ trả về
//! ngay), còn IPC U-mode hiện có là `NBSend`+`NBRecv` polling. Ba mảnh còn thiếu — và là toàn bộ
//! phạm vi KT-280:
//!
//! 1. `sepc += 4` khi tác vụ chặn trong đa nhiệm (`plat-riscv64/src/user.rs`, nhánh `Switched`);
//! 2. nạp `a0..a6` từ `Machine::tcb_ipc_area` khi resume (`user::apply_ipc_return`);
//! 3. scheduler giữ tác vụ đang chặn **sống** và resume nó (ảnh này).
//!
//! # KT-282 — vì sao thêm (khảo sát §1 của `JOURNAL/kt282-mrbuf.md`)
//!
//! `spec/abi.toml` `[message]` đã chốt `max_words = 64`, `words_in_registers = 4`,
//! `words_in_ipc_buffer = 60`, và `[calling.riscv64].ipc_buffer` ghi *"Frame bind qua
//! Tcb.Configure; header chứa receive window"*. Nhưng trước KT-282:
//!
//! * `Tcb.Configure` của core **chỉ** bind CSpace (`tcb_configure(tcb, cspace)`) — tham số
//!   `ipc_buffer?` của `[method_signature."Tcb.Configure"]` **chưa** có;
//! * `build_message` từ chối `length > 4` (`InvalidArgument`) — nợ K3.5 ghi ở đầu
//!   `vifast-core/src/syscall.rs`;
//! * trang IPC buffer 4 KiB **đã** được loader map (`0x4002_0000`, `R|W|U|NX`) nhưng chỉ 10 word
//!   **header** được dùng (KT-67b) — vùng MR4..MR63 bỏ trống.
//!
//! KT-282 đóng nợ đó: `Tcb.Configure` bind **frame 4 KiB + VA**, `length > 4` ⇒ MR4..MR63 đọc/ghi
//! ở **offset 80** của buffer (`[TK]` — spec không chốt offset, xem `vifast_core::ipc`).
//!
//! **KT-311** — trả nợ KT-290 §7.1: **`Tcb.Configure` kênh đầy đủ**. `length = 3` + `MR2` =
//! **mask** tham số mở rộng; giá trị `cspace?`/`vspace?`/`entry?`/`priority?` nằm trong **vùng
//! riêng** của IPC buffer (byte 608..640, `[TK]` KT-311 — **không** đổi ABI, **không** đổi
//! `MessageInfo` 35 bit). TAP 6 đo đường đó; bốn ca âm (entry không căn 2 B ⇒ 6, priority ngoài
//! dải ⇒ 5, cspace không phải CNode ⇒ 2, vspace chưa cài ⇒ 18) nằm trong cùng boot.
//!
//! # Mười TAP (kế hoạch `1..10`)
//!
//! 1. `ipc: umode call` — client U-mode gọi `Call(EP, {label=0x280,length=8}, MR0..MR3)`:
//!    core đưa TCB client sang `BlockedOnReply`, message **nằm trong hàng Endpoint**
//!    (`ep_len=1`, `ep_mr=0x1234`), rồi server U-mode `Recv` **nhận đúng** message đó (label/MR
//!    đọc từ `tcb_ipc_area` của server) và in `# ipc: server ran nr=1`.
//! 2. `ipc: server replied` — server `Send` trên cap `Reply` ⇒ client **được resume** với
//!    `a0=0`, `a2` = `MessageInfo{label=0x281,length=8}`, `a3=0x5a5a`, `a4=1` (đọc từ vùng nhận
//!    IPC của client, không phải kernel tự khai) ⇒ client in `# ipc: umode call ok reply=23130`;
//!    `Reply.caller` sạch, `check_ipc`/`check_caps` xanh, hai tác vụ `Exit(0)`.
//! 3. `ipc: mr4..mr7 via buffer` — **KT-282**: hai vòng `Call`/`Reply` mang **8 MR** rồi **64 MR**
//!    (MR0..MR3 thanh ghi + MR4..MR63 **IPC buffer**); kernel đọc **độc lập** `tcb_ipc_area` của
//!    cả hai bên sau mỗi vòng (`# ipc mr: round=…`) và chương trình U-mode tự đọc buffer của mình
//!    (`# ipc: mr count=8 mr7=0x…`, `# ipc: mr count=64 mr63=0x…`).
//! 4. `ipc: tcb configure via invocation` — **KT-290**: chính client U-mode gọi
//!    `Call(cap TCB, {label = TCB_CONFIGURE, length = 2}, MR0 = CPTR Frame, MR1 = VA)` ⇒ kernel
//!    bind IPC buffer rồi trả `(0, 0)`; client in `# ipc: configure ok va=0x…` +
//!    `# ipc: configure err=0`; kernel đo **độc lập** `delta.configure = 3`,
//!    `delta.denied = 2`, `sys_err(0) = 0`, binding đúng VA (`# ipc configure: …`). Ba ca âm
//!    trong cùng boot: cap không phải TCB ⇒ 18, method id sai ⇒ 18, buffer ngoài frame ⇒
//!    `Configure` 0 nhưng `Call` 8 MR fail-closed 5 (`# ipc: configure neg …`).
//! 5. `ipc: cap transfer` — **KT-309**: cap `Frame` đi qua **vùng dự phòng** của IPC buffer
//!    (offset 560) từ CSpace client sang CSpace server; kernel kiểm `RIGHT_GRANT`, cài cap vào
//!    slot server khai, server **dùng** cap bằng `CNode.Copy` (`# ipc caps: …`).
//! 6. `ipc: tcb configure full` — **KT-311**: `Call(cap TCB, {label = Tcb.Configure, length = 3},
//!    MR0 = CPTR Frame, MR1 = VA, MR2 = mask)`; `cspace?`/`entry?`/`priority?` đọc từ vùng
//!    608..640 của IPC buffer. Kernel đo **độc lập** `entry` (= entry ELF client), `priority`,
//!    `cspace` (đã trả về CSpace gốc), `calls = 2`, `denied = 5` (`# ipc: configure2 …`); client
//!    in `# ipc: configure2 ok=1 …` + `# ipc: configure2 neg …`, và **chứng minh** `cspace?` thật
//!    sự đổi CSpace bằng `Call` trên ô **chỉ có** ở `cnode_alt` (18 thay vì 1).
//! 7. `ipc: tcb resume` — **KT-314**: `Call(cap TCB đích, {label = Tcb.Resume, length = 0})` ⇒
//!    core kiểm **trạng thái trước** rồi `TcbData.entry`, chuyển `Running{hart của caller}` và
//!    **giao `entry`** cho arch qua hook `SyscallEnv::tcb_resume` (D2-83: core không ghi `sepc`).
//!    Bằng chứng: `resume_entry` của **chính tác vụ đích** in `# ipc: resume ran entry=0x…` với
//!    `tf.sepc = 0` ⇒ chỉ hook arch nạp được `sepc`; kèm ba ca âm (`entry = 0` ⇒ 5, tác vụ
//!    `Running` ⇒ 17, tác vụ đã `Exit` ⇒ 17).
//! 8. `ipc: tcb configure vspace` — **KT-320**: `Call(cap TCB, {label = Tcb.Configure, length = 3},
//!    MR0 = CPTR Frame, MR1 = VA, MR2 = `MASK_VSPACE`)` với `vspace?` (word 1 của vùng 608..640) =
//!    CPTR cap **`VSpace`** trong CSpace caller ⇒ core kiểm **loại** rồi lưu **danh tính** vào
//!    `TcbData.vspace` (D2-83: PTE/`satp` thuộc arch), rồi **giao** cho arch qua hook
//!    `SyscallEnv::tcb_vspace`; arch **tiêu thụ** ở lần vào U-mode **kế tiếp** của **đúng** TCB đó
//!    (`# ipc: vspace ok obj=… applied=… match=1`). Ca âm: mask có **bit lạ** ⇒ `InvalidArgument`
//!    **5**; CPTR là cap **`Frame`** ⇒ `WrongObjectType` **2** (không nhận Frame làm VSpace).
//! 9. `ipc: tcb suspend` — **KT-319**: `Call(cap TCB đích, {label = Tcb.Suspend, length = 1},
//!    MR0 = mode)` (`0` = `Abort`). Đường xanh **hai** lần: (a) đích **`Ready`** → `Inactive`,
//!    kernel đo **độc lập** `state = Inactive suspended = 0` (`Tcb.ReadState` — cờ `suspended`
//!    **không** bật: đây là đường invocation, khác model `kill` của KT-65); (b) đích **`Running`**
//!    (ô 15, TAP 7 vừa `Resume`) → `Inactive` rồi `Resume` lại ⇒ `(0, entry)` và tác vụ **chạy lại
//!    thật** — mốc thật của KT-319: `TcbData.entry` **giữ** qua `Suspend`. **Sáu** ca âm trong
//!    cùng boot: `mode = Defer` ⇒ 5, arity 0/2 ⇒ 5, tác vụ `Inactive` ⇒ 17, tác vụ
//!    `BlockedOnSend` ⇒ 17 (bảng 11×11 **cho** `BlockedOnSend → Inactive` ⇒ chỉ kiểm trạng thái
//!    chặn được), thiếu `RIGHT_WRITE` ⇒ 3.
//! 10. `ipc: tcb configure vspace clear + hai tcb` — **KT-325**: (a) `vspace?` với CPTR **rỗng**
//!    (`0`) = **xoá** ⇒ `TcbData.vspace = 0` ("dùng VSpace của ảnh đã nạp") — đường **quay lui**
//!    mà KT-320 chưa có; TCB đó chạy thật **sau** khi xoá ⇒ `vspace_applied` của nó `None`
//!    (arch **không** áp VSpace cũ nữa); (b) **hai TCB — hai VSpace khác nhau** ⇒
//!    `vspace_applied(t1)` và `vspace_applied(t2)` **khác nhau** (bản ghi **theo TCB**, không đè
//!    lên nhau — nợ §9.4 của KT-320 đã đóng). TCB thứ hai có root khác ảnh đã nạp ⇒ arch
//!    **từ chối** vào U-mode (fail-closed) nhưng bản ghi của nó vẫn là VSpace **B**.
//!
//! **Ca âm** (trong cùng boot, chương trình client tự kiểm): `Call` trên ô rỗng ⇒ 1
//! (`InvalidCapability`); `Call` trên cap `Reply` (không phải Endpoint) ⇒ 18 (`NotSupported`);
//! **KT-282** `Call` với `length = 65` (vượt `max_words = 64`) ⇒ 5 (`InvalidArgument`) — không
//! tràn vùng 60 word của buffer.
//!
//! Boot kết thúc bằng **một** `PANIC` có chủ đích (khuôn runner KT-54).
#![no_std]
#![no_main]

use core::fmt::Write;

use vifast_core::{
    KError, Machine, NonReadyTarget, ObjId, ObjType, SlotRef, TcbState, RIGHTS_ALL, RIGHT_GRANT,
};
use vifast_plat_common::obj::{CapId, ObjKind};
use vifast_plat_common::{PanicHandler, Platform, PlatformWriter};
use vifast_plat_riscv64::core_bridge;
use vifast_plat_riscv64::mem_bridge;
use vifast_plat_riscv64::obj as aobj;
use vifast_plat_riscv64::trap::{TrapFrame, REG_SP};
use vifast_plat_riscv64::user::{self, LoadedInit, Stop};
use vifast_plat_riscv64::{read_trap_vector, trap_vector, Riscv64Virt};

/// **KT-322** — rig (dựng `Machine` + cấp cap) tách sang module riêng để giữ **R1**
/// (≤ 1.500 dòng). Cùng nội dung, chỉ dời chỗ.
#[path = "ktest_ipc_rig.rs"]
mod rig;
use rig::{area_of, build_rig, run_turn, set_running, state_of, Rig};

static PLATFORM: Riscv64Virt = Riscv64Virt;
static PANIC: PanicHandler<Riscv64Virt> = PanicHandler::new(Riscv64Virt);

/// Hai chương trình U-mode **riêng** do `build.rs` sinh — hai tiến trình, hai VSpace, hai cspace.
static CLIENT_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ipc_client.elf"));
static SERVER_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ipc_server.elf"));

/// Ô tác vụ (`user::MAX_PROCS`) của hai tiến trình — dùng cho bảng dải VA + capture output.
const SLOT_CLIENT: usize = 1;
const SLOT_SERVER: usize = 2;

/// Ô cspace **của từng tiến trình** (kernel dựng; chương trình U-mode hardcode cùng số).
const CS_SELF: usize = 0;
const CS_EP: usize = 1;
const CS_REPLY: usize = 2;
/// **KT-290** — cap `Tcb` của chính client: `Call` với `label = Tcb.Configure`.
const CS_TCB: usize = 3;
/// **KT-290** — cap `Frame` 4 KiB làm IPC buffer: `MR0` của `Tcb.Configure`.
const CS_FRAME: usize = 4;
/// **KT-309** — cap `Frame` **có** `RIGHT_GRANT` — nguồn của extra cap (client gửi sang server).
const CS_FRAME_GRANT: usize = 5;
/// **KT-309** — cap `Frame` **thiếu** `RIGHT_GRANT` — ca âm (i): gửi ⇒ `InsufficientRights` (3).
const CS_FRAME_NO_GRANT: usize = 6;
/// **KT-309** — slot đích **trống** trong cspace server (server khai `dst` = ô này).
const CS_SRV_DST: usize = 5;
/// **KT-309** — slot đích **bận** trong cspace server (ô 1 = cap `Endpoint`) — ca âm (iii).
const CS_SRV_DST_BUSY: usize = 1;
/// **KT-309** — slot server **copy** cap nhận được sang (chứng minh cap dùng được thật).
const CS_SRV_COPY: usize = 6;
/// **KT-311** — slot trong cspace **client** giữ cap `CNode` của **CSpace thứ hai**
/// (`cnode_alt`) — `cspace?` đường xanh.
const CS_ALT_CNODE: usize = 7;
/// **KT-311** — slot trong `cnode_alt` giữ cap tới **CSpace gốc** (client trả `cspace` về đây).
const CS_ALT_BACK: usize = 0;
/// **KT-311** — slot trong `cnode_alt` giữ cap `Tcb` của client (gọi `Configure` khi CSpace đang
/// là `cnode_alt`).
const CS_ALT_TCB: usize = 3;
/// **KT-311** — slot trong `cnode_alt` giữ cap `Reply` — **chỉ có** ở đó, nên `Call` trên nó trả
/// **18** khi CSpace đã đổi và **1** khi chưa (chứng minh `cspace?` thật sự đổi CSpace).
const CS_ALT_ONLY: usize = 8;
/// **KT-314** — cap `Tcb` của **server** (đã `Exit`) trong cspace client — ca âm "resume tác vụ
/// đã kết thúc" ⇒ `IllegalOperation` (**17**).
const CS_TCB_S: usize = 14;
/// **KT-314** — cap `Tcb` **đích** (đường xanh `Configure(entry?)` + `Resume`).
const CS_TCB_T: usize = 15;
/// **KT-314** — cap `Tcb` **đích thứ hai** (`entry = 0`) — ca âm `InvalidArgument` (5).
const CS_TCB_T2: usize = 16;
/// **KT-320** — cap **`VSpace`** của client trong cspace **của client** (`MR1` của
/// `Tcb.Configure` tham số `vspace?`). Ô 4 (`Frame`) là ca âm "vspace không phải VSpace".
const CS_VSPACE: usize = 17;
/// **KT-325** — cap **`VSpace` thứ hai`** (object layer **thật**, root khác ảnh đã nạp) — TAP 10
/// cấu hình nó cho TCB thứ hai ⇒ `vspace_applied(tcb2)` **khác** `vspace_applied(tcb1)`.
const CS_VSPACE2: usize = 21;
// KT-319 (merge vòng 22): bốn ô dưới đây **dời** 17/18/19 → 18/19/20 để không đụng
// `CS_VSPACE = 17` của KT-320 (cùng vòng, hai nhánh độc lập) — khớp `ipc_client_user.rs`.
/// **KT-319** — cap `Tcb` **đích thứ ba** (rig để `Ready`): `Suspend` ⇒ `(0, 0)`; kernel đo
/// **độc lập** `state = Inactive suspended = 0` (đích **không** được `Resume` ⇒ `entry = 0`).
const CS_TCB_T3: usize = 18;
/// **KT-319** — cap `Tcb` **chỉ đọc** (`RIGHT_READ`, mint từ cap đích thứ ba): ca âm quyền —
/// `Suspend` thiếu `RIGHT_WRITE` ⇒ `InsufficientRights` (3).
const CS_TCB_RO: usize = 19;
/// **KT-319** — cap `Tcb` **đích thứ tư** — rig để `BlockedOnSend`: bảng 11×11 **cho**
/// `BlockedOnSend → Inactive`, nên **chỉ** kiểm trạng thái của đường invocation mới chặn được
/// (đột biến "bỏ kiểm trạng thái" sẽ treo tác vụ đang chờ IPC ⇒ TAP 8 đỏ).
const CS_TCB_T4: usize = 20;
/// **KT-319** — slot trong cspace **server** giữ cap `Endpoint` **riêng** của đích thứ tư (không
/// đụng `EP` của luồng `Call`/`Reply` — TAP 1..7 giữ nguyên).
const CS_SRV_EP2: usize = 12;
/// **KT-329** — ba TCB đích của TAP 11 trong cspace **client** (ô 22/24/26; `SchedContext` ở
/// 23/25/27 do rig giữ). `T5` = đường xanh (a)(b)(c); `T6` = `Resume` **trước khi chạy** (đột
/// biến ii); `T7` = chạy lại **không** qua `Resume` (đột biến iii).
const CS_TCB_T5: usize = 22;
const CS_TCB_T6: usize = 24;
const CS_TCB_T7: usize = 26;
const CS_SC_T5: usize = 23;
const CS_SC_T6: usize = 25;
const CS_SC_T7: usize = 27;

/// Ô trong CNode gốc của rig.
const RIG_UNTYPED: usize = 0;
const RIG_TCB_C: usize = 1;
const RIG_TCB_S: usize = 2;
const RIG_EP: usize = 3;
const RIG_REPLY: usize = 4;
const RIG_CTL: usize = 5;
const RIG_SC_C: usize = 6;
const RIG_SC_S: usize = 7;
const RIG_CSPACE: usize = 8;
/// **KT-282** — cap `Frame` 4 KiB của **IPC buffer** (bind qua `Tcb.Configure`).
const RIG_IPC_BUF: usize = 9;
/// **KT-314** — TCB **đích** (rig để `Inactive`, chưa từng chạy): `Tcb.Resume` đưa nó lên
/// `Running` và arch nạp `sepc` = `TcbData.entry` ở lượt chạy kế tiếp.
const RIG_TCB_T: usize = 10;
/// **KT-314** — TCB **đích thứ hai** (`Inactive`, `entry = 0`): ca âm `InvalidArgument` (5).
const RIG_TCB_T2: usize = 11;
/// **KT-314** — `SchedContext` cho hai TCB đích (bất biến KT-20: `Running ⇒ còn budget`).
const RIG_SC_T: usize = 12;
const RIG_SC_T2: usize = 13;
/// **KT-319** — TCB **đích thứ ba** (`Ready`, chỉ bị `Suspend` — không bao giờ chạy) + SC của nó
/// (ô 14/15 là hai ô cuối của CNode gốc 16 ô).
const RIG_TCB_T3: usize = 14;
const RIG_SC_T3: usize = 15;

/// Label/MR **hợp đồng** giữa hai chương trình U-mode (không phải ABI) — khớp
/// `init/ipc_client_user.rs` + `init/ipc_server_user.rs`.
const LABEL_REQ: u32 = 0x280;
const LABEL_REPLY: u32 = 0x281;
const ARG: u64 = 0x1234;
const REPLY_VALUE: u64 = 0x5a5a;
/// **KT-282** — MR4..MR7 của request (vòng 1) và của reply.
const REQ_MR4: u64 = 0x2820_0004;
const REQ_MR7: u64 = 0x2820_0007;
const REPLY_MR4: u64 = 0x2821_0004;
const REPLY_MR7: u64 = 0x2821_0007;
/// **KT-282** — MR63 (word cuối của vùng buffer) của request/reply vòng 2 (`length = 64`).
const REQ_MR63: u64 = 0x2820_003f;
const REPLY_MR63: u64 = 0x2821_003f;

/// Ưu tiên: server **cao hơn** client để sau `Reply` core chọn **server** chạy tiếp (server còn
/// một syscall `Exit`), rồi kernel mới resume client. Không phụ thuộc thứ tự FIFO.
const PRIO_CLIENT: u8 = 3;
const PRIO_SERVER: u8 = 5;

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    PLATFORM.put_str("TAP version 13\n1..11\n");
    PLATFORM.set_trap_vector(trap_vector());
    let stvec = read_trap_vector();
    let _mmu = vifast_plat_riscv64::mmu::init();
    let _tlb = vifast_plat_riscv64::tlb::init();
    let obj = vifast_plat_riscv64::obj::init();
    let core = core_bridge::init(obj.arena_end, core_bridge::CORE_ARENA_BYTES);
    if !user::init_syscall_stacks() {
        PLATFORM.put_str("not ok 1 - ipc: umode call: khong cap duoc stack syscall\n");
        PLATFORM.put_str("# ktest halt\n");
        PLATFORM.halt(1)
    }
    // Đa nhiệm bật để `Exit`/`Yield` **trả về kernel** (như `ktest-proc`), và để nhánh
    // `Switched` (tác vụ chặn) nhường CPU thay vì PANIC.
    user::enable_multitask(true);
    user::capture_begin();

    // ---- nạp HAI chương trình U-mode vào hai VSpace (cùng Untyped của ảnh test).
    let client = match user::load_init_slot(CLIENT_ELF, obj.untyped0, SLOT_CLIENT) {
        Ok(loaded) => loaded,
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 1 - ipc: umode call: nap client that bai kind={} ({:?})",
                error.name(),
                error
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    let server = match user::load_init_slot(SERVER_ELF, obj.untyped0, SLOT_SERVER) {
        Ok(loaded) => loaded,
        Err(error) => {
            let mut w = PlatformWriter(&PLATFORM);
            let _ = writeln!(
                w,
                "not ok 1 - ipc: umode call: nap server that bai kind={} ({:?})",
                error.name(),
                error
            );
            PLATFORM.put_str("# ktest halt\n");
            PLATFORM.halt(1)
        }
    };
    let rig =
        core_bridge::with_machine(|m| build_rig(m, core.untyped, obj.untyped0, client.vspace));
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc rig: stvec=0x{:x} untyped={} core_untyped={} cspace_c={} cspace_s={} cspace_alt={} tcb_c={} tcb_s={} ep={} reply={} ipc_buf={}",
            stvec,
            obj.untyped0.0,
            core.untyped,
            rig.cnode_c,
            rig.cnode_s,
            rig.cnode_alt,
            rig.tcb_c,
            rig.tcb_s,
            rig.ep,
            rig.reply,
            rig.ipc_buf
        );
    }

    // ================================================================ lượt 1: client `Call` (8 MR)
    // **KT-290** — mốc số đếm invocation TRƯỚC lượt 1: lượt này chứa **ba** `Tcb.Configure`
    // (bind đúng, bind ngoài frame, bind lại) + **hai** ca âm (cap không phải TCB, method sai).
    let inv_before = vifast_core::invoke::stats();
    let mut tf_c = TrapFrame::zeroed();
    tf_c.x[REG_SP] = user::USER_STACK_TOP;
    tf_c.sepc = client.entry;
    let stop1 = run_turn(&client, &mut tf_c, rig.tcb_c, SLOT_CLIENT);
    let state1 = state_of(rig.tcb_c);
    // **KT-290** — đo **độc lập** ở kernel: số `Tcb.Configure` được phép/bị từ chối, lỗi `Call`
    // cuối cùng đã trả về U-mode (`sys_err(0)`), và **binding** thật của TCB client (do chính
    // invocation ghi — rig không bind hộ).
    let inv_after = vifast_core::invoke::stats();
    let cfg_delta = inv_after.configure.saturating_sub(inv_before.configure);
    let denied_delta = inv_after.denied.saturating_sub(inv_before.denied);
    let cfg_err = user::sys_err(0);
    let binding = core_bridge::with_machine(|m| m.tcb_ipc_buffer_binding(rig.tcb_c).ok().flatten());
    let bound = binding.is_some();
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc configure: method={} delta={} denied={} bound={} va=0x{:x}",
            vifast_core::invoke::TCB_CONFIGURE,
            cfg_delta,
            denied_delta,
            u8::from(bound),
            binding.map(|b| b.va).unwrap_or(0)
        );
    }
    // seL4: `Call` khi **chưa** có người nhận ⇒ caller chặn ở **Send** (`call = true`), message
    // nằm trong hàng Endpoint. Chuyển sang `BlockedOnReply` chỉ khi server `Recv` lấy message.
    let blocked_send = matches!(state1, Some(TcbState::BlockedOnSend { call: true, .. }));
    let (ep_len, ep_mr) = core_bridge::with_machine(|m| {
        (
            m.endpoint_send_len(rig.ep).ok(),
            m.endpoint_last_mr(rig.ep).ok().flatten(),
        )
    });
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc call: blocked={} state={} call=1 ep_len={} ep_mr=0x{:x} label=0x{:x} stop={}",
            u8::from(blocked_send),
            state1.map(tcb_state_name).unwrap_or("none"),
            ep_len.unwrap_or(usize::MAX),
            ep_mr.unwrap_or(0),
            LABEL_REQ,
            stop_name(stop1)
        );
    }

    // ================================================================ lượt 2: server `Recv` + `Yield`
    let mut tf_s = TrapFrame::zeroed();
    tf_s.x[REG_SP] = user::USER_STACK_TOP;
    tf_s.sepc = server.entry;
    let stop2 = run_turn(&server, &mut tf_s, rig.tcb_s, SLOT_SERVER);
    let state_s2 = state_of(rig.tcb_s);
    // **Bằng chứng trực tiếp** của hợp đồng seL4: sau `Recv` của server, caller chuyển
    // `BlockedOnSend{call}` → `BlockedOnReply` (server đang giữ `Reply.caller`).
    let state_c2 = state_of(rig.tcb_c);
    let blocked_reply = matches!(state_c2, Some(TcbState::BlockedOnReply { .. }));
    // Message **server nhận được** — đọc từ vùng nhận IPC của server (core sở hữu, D2-83).
    let (srv_label, srv_mr) = core_bridge::with_machine(|m| {
        m.tcb_ipc_area(rig.tcb_s)
            .ok()
            .and_then(|area| {
                area.info
                    .map(|info| (info.label(), area.mr(0).unwrap_or(0)))
            })
            .unwrap_or((u32::MAX, 0))
    });
    // **KT-282** — MR4..MR7 của request vòng 1 (buffer của server).
    let srv1 = area_of(rig.tcb_s);
    let caller_mid = core_bridge::with_machine(|m| m.reply_caller(rig.reply).ok().flatten());
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc server: recv={} label=0x{:x} mr=0x{:x} client={} caller={} server={} stop={}",
            u8::from(srv_label == LABEL_REQ && srv_mr == ARG),
            srv_label,
            srv_mr,
            state_c2.map(tcb_state_name).unwrap_or("none"),
            caller_mid.map(|t| t as u64).unwrap_or(0),
            state_s2.map(tcb_state_name).unwrap_or("none"),
            stop_name(stop2)
        );
    }

    // ================================================================ lượt 3: server `Reply` (8 MR) + `Yield`
    let stop3 = run_turn(&server, &mut tf_s, rig.tcb_s, SLOT_SERVER);
    let state_s3 = state_of(rig.tcb_s);
    let state_c3 = state_of(rig.tcb_c);
    let reply_caller = core_bridge::with_machine(|m| m.reply_caller(rig.reply).ok().flatten());
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc reply: client={} reply_caller={} server={} stop={}",
            state_c3.map(tcb_state_name).unwrap_or("none"),
            reply_caller.map(|t| t as u64).unwrap_or(0),
            state_s3.map(tcb_state_name).unwrap_or("none"),
            stop_name(stop3)
        );
    }

    // ================================================================ lượt 4: resume client (8 MR)
    // Nửa thứ hai của hợp đồng resume: `a0..a6` + **MR4..MR7 trong IPC buffer** lấy từ vùng nhận
    // IPC của client (message server vừa giao). Không có message ⇒ `false` và client **không**
    // được resume (đỏ có chẩn đoán). Client kiểm reply rồi `Call` **64 MR** ⇒ chặn tiếp.
    let applied = user::apply_ipc_return(&mut tf_c, rig.tcb_c);
    let _stop4 = if applied {
        run_turn(&client, &mut tf_c, rig.tcb_c, SLOT_CLIENT)
    } else {
        None
    };
    let state_c4 = state_of(rig.tcb_c);
    // Message **client nhận được** vòng 1 — kernel đọc độc lập (không tin lời chương trình).
    let (cli_label, cli_mr0, cli_mr1) = core_bridge::with_machine(|m| {
        m.tcb_ipc_area(rig.tcb_c)
            .ok()
            .and_then(|area| {
                area.info.map(|info| {
                    (
                        info.label(),
                        area.mr(0).unwrap_or(0),
                        area.mr(1).unwrap_or(0),
                    )
                })
            })
            .unwrap_or((u32::MAX, 0, 0))
    });
    let cli1 = area_of(rig.tcb_c);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc mr: round=1 srv_len={} srv_mr4=0x{:x} srv_mr7=0x{:x} cli_len={} cli_mr4=0x{:x} cli_mr7=0x{:x}",
            srv1.map(|a| a.0 as u64).unwrap_or(u64::MAX),
            srv1.map(|a| a.1).unwrap_or(0),
            srv1.map(|a| a.2).unwrap_or(0),
            cli1.map(|a| a.0 as u64).unwrap_or(u64::MAX),
            cli1.map(|a| a.1).unwrap_or(0),
            cli1.map(|a| a.2).unwrap_or(0)
        );
    }
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc client: reply_label=0x{:x} reply_mr0=0x{:x} reply_mr1={} const:label_reply=0x{:x} const:reply_value=0x{:x}",
            cli_label, cli_mr0, cli_mr1, LABEL_REPLY, REPLY_VALUE
        );
    }

    // ================================================================ lượt 5: server `Recv` (64 MR) + `Reply` (64 MR) + `Yield`
    // **KT-309**: server **không** `Exit` nữa — nó `Yield` để client gửi message mang **extra
    // cap** (message xếp hàng trên Endpoint, server **chưa** chặn ở `Recv`).
    let applied_s = user::apply_ipc_return(&mut tf_s, rig.tcb_s);
    let _stop5 = if applied_s {
        run_turn(&server, &mut tf_s, rig.tcb_s, SLOT_SERVER)
    } else {
        None
    };
    let state_s5 = state_of(rig.tcb_s);
    let srv2 = area_of(rig.tcb_s);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc mr: round=2 srv_len={} srv_mr4=0x{:x} srv_mr63=0x{:x}",
            srv2.map(|a| a.0 as u64).unwrap_or(u64::MAX),
            srv2.map(|a| a.1).unwrap_or(0),
            srv2.map(|a| a.2).unwrap_or(0)
        );
    }

    // ================================================================ lượt 6: resume client (64 MR) + ca âm
    let applied2 = user::apply_ipc_return(&mut tf_c, rig.tcb_c);
    let _stop6 = if applied2 {
        run_turn(&client, &mut tf_c, rig.tcb_c, SLOT_CLIENT)
    } else {
        None
    };
    let _state_c6 = state_of(rig.tcb_c);
    let cli2 = area_of(rig.tcb_c);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc mr: round=2 cli_len={} cli_mr4=0x{:x} cli_mr63=0x{:x}",
            cli2.map(|a| a.0 as u64).unwrap_or(u64::MAX),
            cli2.map(|a| a.1).unwrap_or(0),
            cli2.map(|a| a.2).unwrap_or(0)
        );
    }
    // ================================================================ lượt 7: server — **KT-309** cap transfer
    // Server **không** được resume từ `Recv` (nó `Yield` ở lượt 5) ⇒ **không** gọi
    // `apply_ipc_return` (sẽ áp lại message 64 MR cũ — stale).
    let cap_before = core_bridge::with_machine(|m| (m.extra_caps_sent(), m.extra_caps_installed()));
    let stop7 = run_turn(&server, &mut tf_s, rig.tcb_s, SLOT_SERVER);
    let state_s7 = state_of(rig.tcb_s);
    let cap_after = core_bridge::with_machine(|m| (m.extra_caps_sent(), m.extra_caps_installed()));
    let cap_sent = cap_after.0.saturating_sub(cap_before.0);
    let cap_recv = cap_after.1.saturating_sub(cap_before.1);
    // Cap **thật** trong CSpace server ở slot đích + slot copy — kernel đọc **độc lập** (không
    // tin lời chương trình): cùng object, cùng loại `Frame`, quyền đã **bỏ `RIGHT_GRANT`**.
    let srv_cap = core_bridge::with_machine(|m| {
        let cspace = m.tcb_info_obj(rig.tcb_s).ok()?.cspace?;
        let cap = m.cap_at(cspace, CS_SRV_DST).ok()?;
        let info = m.cap_info(cap).ok()?;
        Some((info.obj, info.ty, info.rights, info.alive))
    });
    let srv_copy = core_bridge::with_machine(|m| {
        let cspace = m.tcb_info_obj(rig.tcb_s).ok()?.cspace?;
        let cap = m.cap_at(cspace, CS_SRV_COPY).ok()?;
        let info = m.cap_info(cap).ok()?;
        Some((info.obj, info.ty, info.rights, info.alive))
    });
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc caps: sent={} received={} slot={} rights=0x{:x} obj={} ty={} alive={} copy_slot={} copy_obj={} copy_ty={} same={}",
            cap_sent,
            cap_recv,
            CS_SRV_DST,
            srv_cap.map(|c| c.2).unwrap_or(0),
            srv_cap.map(|c| c.0 as u64).unwrap_or(u64::MAX),
            srv_cap.map(|c| obj_type_name(c.1)).unwrap_or("none"),
            srv_cap.map(|c| u8::from(c.3)).unwrap_or(0),
            CS_SRV_COPY,
            srv_copy.map(|c| c.0 as u64).unwrap_or(u64::MAX),
            srv_copy.map(|c| obj_type_name(c.1)).unwrap_or("none"),
            u8::from(matches!((srv_cap, srv_copy), (Some(a), Some(b)) if a.0 == b.0 && a.1 == b.1))
        );
    }

    // ================================================================ lượt 8: resume client — kiểm reply cap + KT-311
    // **KT-311** — mốc số đếm invocation TRƯỚC lượt 8: lượt này chứa **sáu** `Tcb.Configure`
    // kênh đầy đủ (bốn ca âm + đường xanh + trả `cspace` về) rồi client `Yield` ⇒ đo **độc lập**
    // ở kernel **ngay sau** pha đó (trước các ca âm cũ của lượt 9).
    let inv_before2 = vifast_core::invoke::stats();
    let applied3 = user::apply_ipc_return(&mut tf_c, rig.tcb_c);
    let stop8 = if applied3 {
        run_turn(&client, &mut tf_c, rig.tcb_c, SLOT_CLIENT)
    } else {
        None
    };
    let state_c8 = state_of(rig.tcb_c);
    let inv_after2 = vifast_core::invoke::stats();
    let cfg2_calls = inv_after2.configure.saturating_sub(inv_before2.configure);
    let cfg2_denied = inv_after2.denied.saturating_sub(inv_before2.denied);
    // **KT-311** — đo **độc lập** ở kernel: `entry`/`priority`/`cspace` **thật** của TCB client
    // sau lượt U-mode (client đã trả `cspace` về CSpace gốc; `entry`/`priority` giữ nguyên).
    let cfg2_entry = core_bridge::with_machine(|m| m.tcb_entry(rig.tcb_c).ok());
    let cfg2_info = core_bridge::with_machine(|m| m.tcb_info_obj(rig.tcb_c).ok());
    let cfg2_prio = cfg2_info.map(|i| i.priority);
    let cfg2_cspace = cfg2_info.and_then(|i| i.cspace);
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc: configure2 entry=0x{:x} prio={} cspace={} vspace=0 calls={} denied={} ok=1",
            cfg2_entry.unwrap_or(0),
            cfg2_prio.map(|p| p as u64).unwrap_or(u64::MAX),
            cfg2_cspace.map(|c| c as u64).unwrap_or(u64::MAX),
            cfg2_calls,
            cfg2_denied
        );
    }
    // ================================================================ lượt 9: client — ca âm cũ + `Exit(0)`
    // Client đã `Yield` sau pha KT-311 ⇒ lượt này **không** `apply_ipc_return` (không có message
    // mới; áp lại message cũ sẽ stale — cùng lý do lượt 7 của server).
    //
    // **KT-319** — mốc số đếm invocation **trước** lượt 9: lượt này chứa **hai** `Tcb.Suspend`
    // đường xanh (đích `Ready` + đích `Running`) trên nền năm ca âm (defer/arity 0/arity 2/
    // inactive/quyền) ⇒ `delta.suspend` phải **đúng 2** (không phải "ít nhất 1").
    let inv_before9 = vifast_core::invoke::stats();
    let stop9 = run_turn(&client, &mut tf_c, rig.tcb_c, SLOT_CLIENT);
    let inv_after9 = vifast_core::invoke::stats();
    let suspend_calls = inv_after9.suspend.saturating_sub(inv_before9.suspend);
    let state_c9 = state_of(rig.tcb_c);

    // ================================================================ KT-314: đo `Tcb.Resume` + lượt 10
    // **Đo độc lập ở kernel** ngay sau lượt U-mode: `TcbData.entry` **thật** của TCB đích (client
    // vừa ghi bằng `Configure(entry?)`) và trạng thái **thật** sau `Resume` (`Running`).
    let resume_entry = core_bridge::with_machine(|m| m.tcb_entry(rig.tcb_t).ok());
    let resume_state = state_of(rig.tcb_t);
    // **KT-319** — đo **độc lập** ở kernel `Tcb.ReadState` của TCB đích thứ ba **sau** `Suspend`
    // từ U-mode: `state = Inactive`, `suspended = 0` (đường invocation **không** bật cờ của đường
    // model KT-65), `blocked_on_kind = None`, `exit_code = None` (D2-25: chỉ `Configure`/`Resume`
    // xoá). Đích này **không** được `Resume` ⇒ `entry` vẫn `0` và nó **không** chạy.
    let suspend_read_t3 =
        core_bridge::with_machine(|m| m.tcb_read_state(SlotRef::new(rig.root, RIG_TCB_T3)).ok());
    // Đọc **lần thứ hai** bằng cửa khác (`tcb_state`) — hai accessor phải **khớp**; lệch ⇒ TAP 8
    // đỏ (không tin một cửa duy nhất).
    let suspend_state_t3 = state_of(rig.tcb_t3);
    // **KT-319** — ca âm `BlockedOnSend` **không** được đổi trạng thái: đích thứ tư vẫn
    // `BlockedOnSend` **sau** lượt U-mode (đo bằng cửa thứ hai, độc lập với lời chương trình).
    let suspend_state_t4 = state_of(rig.tcb_t4);
    // Ca âm đo **độc lập** bằng **cùng** cửa core (`Machine::tcb_resume_invocation`) trên TCB
    // đích thứ ba (`entry = 0`, vừa bị `Suspend` ⇒ `Inactive`): phải là `InvalidArgument` (5) và
    // trạng thái **không** đổi.
    //
    // **KT-325** — ca âm dời từ TCB thứ hai sang TCB thứ ba: TCB thứ hai nay là **T0** của TAP 10
    // và client ghi `entry?` cho nó ở pha 7d (sau lượt 9) ⇒ `entry` **không còn 0** nên tiền đề
    // "`entry = 0` ⇒ 5" **hết hiệu lực** với nó (`k6-test-invariants.md` §4.1). TCB thứ ba vẫn
    // `entry = 0` (chỉ `vspace?` = B được cấu hình) ⇒ ca âm **giữ nguyên ngữ nghĩa**.
    let neg_noentry = core_bridge::with_machine(|m| {
        m.tcb_resume_invocation(SlotRef::new(rig.root, RIG_TCB_T3), 0)
            .err()
            .map(|e| e.id())
    });
    let neg_state = state_of(rig.tcb_t3);
    // **KT-320** — `vspace?`: đọc **độc lập ở kernel** `TcbData.vspace` của TCB đích (client vừa
    // ghi qua `Tcb.Configure` trong lượt 9) **trước** lượt 10 — lượt 10 là lần vào U-mode **kế
    // tiếp** của đúng TCB đó, nơi arch **tiêu thụ** giá trị (`user_run::apply_vspace`).
    //
    // Thứ tự này do **rig** bảo đảm (gọi `run_turn` tuần tự trên một hart), không phải "thường
    // đúng": không có hart thứ hai nào chạy tác vụ này (D2-72 matrix 1/2/4 nhưng mọi lượt U-mode
    // của ảnh nằm trên hart 0 — `rust_secondary_main` park), nên bất biến **không** phụ thuộc
    // thứ tự tick.
    let cfg_vspace = core_bridge::with_machine(|m| m.tcb_vspace(rig.tcb_t).ok());
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc: resume ok entry=0x{:x} state={}",
            resume_entry.unwrap_or(0),
            resume_state.map(tcb_state_name).unwrap_or("none")
        );
        let _ = writeln!(
            w,
            "# ipc: resume neg noentry={} state={}",
            neg_noentry.map(u64::from).unwrap_or(u64::MAX),
            neg_state.map(tcb_state_name).unwrap_or("none")
        );
    }
    // **KT-319** — một dòng do **kernel** in sau khi đo **độc lập** `Tcb.ReadState` của đích thứ
    // ba (bị `Suspend` từ U-mode, **không** `Resume`) + số `Tcb.Suspend` thành công của lượt 9 +
    // trạng thái đích thứ nhất **sau cặp** `Suspend`/`Resume` (`Running`). `suspended=0` là điểm
    // khác biệt **có chủ đích** với model `kill` (KT-65): đường invocation chuyển `Inactive`,
    // **không** đánh cờ treo.
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc: suspend state={} suspended={} calls={} resumed={} blocked={}",
            suspend_read_t3
                .map(|s| tcb_state_name(s.state))
                .unwrap_or("none"),
            suspend_read_t3
                .map(|s| u8::from(s.suspended))
                .unwrap_or(u8::MAX),
            suspend_calls,
            resume_state.map(tcb_state_name).unwrap_or("none"),
            suspend_state_t4.map(tcb_state_name).unwrap_or("none")
        );
    }
    // **Lượt 10** — chạy **TCB đích** vừa được `Resume`. `tf_t.sepc` **cố ý để `0`**: chỉ hook
    // `SyscallEnv::tcb_resume` (arch) nạp `sepc` từ `TcbData.entry` — nếu không, đích fault ở
    // `0` và TAP 7 đỏ. Đích chạy `resume_entry` (in dấu rồi `Exit(0)`), **không** phải `_start`.
    let mut tf_t = TrapFrame::zeroed();
    tf_t.x[REG_SP] = user::USER_STACK_TOP;
    let stop10 = run_turn(&client, &mut tf_t, rig.tcb_t, SLOT_CLIENT);
    let state_t10 = state_of(rig.tcb_t);
    // **KT-320** — arch **tiêu thụ** `TcbData.vspace` khi vào U-mode: `(obj, root_pa, asid,
    // match)` của lần áp của **đúng TCB này** (API **theo TCB** từ KT-325 — trước đó là "lần áp
    // gần nhất" toàn cục). `match = 1` ⇔ root của VSpace đã cấu hình **khớp** root của ảnh đã
    // nạp ⇒ `satp` lấy **từ VSpace của TCB** (không phải từ ảnh) mà tác vụ vẫn chạy được tới
    // `Exit(0)`.
    //
    // **KT-325** — TCB thứ hai (`rig.tcb_t3`) đã được client arm VSpace **B** ở pha 7d (cùng lượt
    // 9, **trước** lượt này) ⇒ khe chờ của TCB thứ nhất **không** bị đè (nếu khe còn toàn cục,
    // lượt này sẽ không tiêu thụ được A ⇒ TAP 8 đỏ).
    let vspace_applied = vifast_plat_riscv64::user_run::vspace_applied(rig.tcb_t);
    // Bất biến IPC/cap đo **ngay sau** lượt 10 — trước hai lượt mới của KT-325 (lượt 11/12) để
    // TAP 1..9 giữ **cùng** điểm đo như trước KT-325.
    let (check_ipc, check_caps) =
        core_bridge::with_machine(|m| (m.check_ipc().is_ok(), m.check_caps().is_ok()));
    {
        let (ap_obj, ap_root, ap_asid, ap_match) = match vspace_applied {
            Some((obj, root, asid, khop)) => {
                (u64::from(obj), root, u64::from(asid), u8::from(khop))
            }
            None => (0, 0, 0, 0),
        };
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc: vspace ok obj={} applied={} match={} root=0x{:x} asid={}",
            cfg_vspace.map(u64::from).unwrap_or(u64::MAX),
            ap_obj,
            ap_match,
            ap_root,
            ap_asid
        );
    }

    // ================================================================ KT-325: đường xoá + hai TCB
    // Client (lượt 9, pha 7d) đã: (b) cấu hình `vspace?` = VSpace **B** cho TCB thứ hai
    // (`rig.tcb_t3`) — arm **thứ hai** cùng tồn tại với arm của TCB thứ nhất; (a) đặt `entry?` +
    // `vspace?` = VSpace A cho TCB thứ ba (`rig.tcb_t2`) rồi **xoá** `vspace?` (CPTR rỗng).
    //
    // Đo **độc lập ở kernel** trạng thái **sau** lượt 9 (không tin lời chương trình):
    let cfg_vspace2 = core_bridge::with_machine(|m| m.tcb_vspace(rig.tcb_t3).ok());
    let cfg_vspace_cleared = core_bridge::with_machine(|m| m.tcb_vspace(rig.tcb_t2).ok());
    // `entry?` của TCB thứ ba **giữ nguyên** qua đường xoá (`Configure` chỉ xoá `vspace`).
    let entry_cleared = core_bridge::with_machine(|m| m.tcb_entry(rig.tcb_t2).ok());
    // **Lượt 11** — TCB thứ ba chạy **sau khi** `vspace` bị xoá. `tf.sepc` lấy từ `TcbData.entry`
    // (kernel đọc, client ghi) — **không** dùng hook `Resume` để giữ phép đo này **tách** khỏi
    // TAP 7. Nếu arch còn áp VSpace A (đột biến "bỏ nhánh xoá"), bản ghi `applied` của TCB này sẽ
    // `Some`, không phải `None`.
    let mut tf_t0 = TrapFrame::zeroed();
    tf_t0.x[REG_SP] = user::USER_STACK_TOP;
    tf_t0.sepc = entry_cleared.unwrap_or(0);
    let stop_clear = run_turn(&client, &mut tf_t0, rig.tcb_t2, SLOT_CLIENT);
    let applied_cleared = vifast_plat_riscv64::user_run::vspace_applied(rig.tcb_t2);
    let state_cleared = state_of(rig.tcb_t2);
    // **Lượt 12** — TCB thứ hai chạy với VSpace **B**: arch giải ra root **khác** `loaded.root` ⇒
    // **từ chối** vào U-mode (fail-closed, `Stop::Blocked`) nhưng **bản ghi** phải là **B** — khác
    // hẳn bản ghi của TCB thứ nhất (A). Đây là điều chứng minh nợ §9.4 của KT-320 đã đóng.
    let mut tf_t2 = TrapFrame::zeroed();
    tf_t2.x[REG_SP] = user::USER_STACK_TOP;
    let stop_vs2 = run_turn(&client, &mut tf_t2, rig.tcb_t3, SLOT_CLIENT);
    let applied_vs2 = vifast_plat_riscv64::user_run::vspace_applied(rig.tcb_t3);
    let state_vs2 = state_of(rig.tcb_t3);
    // **KT-325** — đọc **lại** bản ghi của TCB thứ nhất **sau** khi TCB thứ hai đã áp: đây mới là
    // chỗ chứng minh **không đè** (nếu `vspace_applied` trả "lần áp gần nhất" toàn cục thì bản ghi
    // của TCB thứ nhất đã thành **B** — đột biến (ii) ⇒ đỏ ở đúng `ap1`).
    let applied_t1_end = vifast_plat_riscv64::user_run::vspace_applied(rig.tcb_t);
    // Bất biến IPC/cap **lần hai**, sau cả hai lượt mới (TAP 10 dùng số này — không dùng số đo
    // sớm hơn của TAP 1..9).
    let (check_ipc2, check_caps2) =
        core_bridge::with_machine(|m| (m.check_ipc().is_ok(), m.check_caps().is_ok()));
    {
        let ap1 = applied_t1_end.map(|(o, _, _, _)| u64::from(o)).unwrap_or(0);
        let ap2 = applied_vs2.map(|(o, _, _, _)| u64::from(o)).unwrap_or(0);
        let ap0 = applied_cleared
            .map(|(o, _, _, _)| u64::from(o))
            .unwrap_or(0);
        let m2 = applied_vs2
            .map(|(_, _, _, k)| u8::from(k))
            .unwrap_or(u8::MAX);
        let diff = u8::from(ap1 != 0 && ap2 != 0 && ap1 != ap2);
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc: vspace2 t1={} t2={} t0={} v1={} v2={} v0={} ap1={} ap2={} ap0={} match2={} diff={} st1={} st2={} st0={}",
            rig.tcb_t,
            rig.tcb_t3,
            rig.tcb_t2,
            cfg_vspace.map(u64::from).unwrap_or(u64::MAX),
            cfg_vspace2.map(u64::from).unwrap_or(u64::MAX),
            cfg_vspace_cleared.map(u64::from).unwrap_or(u64::MAX),
            ap1,
            ap2,
            ap0,
            m2,
            diff,
            // Chẩn đoán (không phải điều kiện): trạng thái core **sau** ba lượt — `st2 = Running`
            // vì arch **từ chối** vào U-mode mà **không** đổi trạng thái core (D2-83); `st0 =
            // Inactive` vì tác vụ đã `Exit(0)`.
            state_t10.map(tcb_state_name).unwrap_or("none"),
            state_vs2.map(tcb_state_name).unwrap_or("none"),
            state_cleared.map(tcb_state_name).unwrap_or("none")
        );
    }
    // ================================================================ KT-329: TAP 11 — `Suspend(Defer)`
    // Toàn bộ phép đo (a)(b)(c) + evidence nằm trong `rig::run_defer_tap` để giữ **R1**
    // (`ktest_ipc.rs` ≤ 1.500 dòng); rig đo **độc lập** ở kernel, không tin lời chương trình.
    let defer_tap = rig::run_defer_tap(&client, &rig);
    user::capture_end();

    // ================================================================ TAP 1..5
    let client_said_call = user::captured_contains(SLOT_CLIENT, b"# ipc: umode call ok reply=");
    let client_said_neg = user::captured_contains(SLOT_CLIENT, b"# ipc: neg empty=1 reply=18 ok=1");
    let client_said_mr8 = user::captured_contains(SLOT_CLIENT, b"# ipc: mr count=8 mr7=0x");
    let client_said_mr64 = user::captured_contains(SLOT_CLIENT, b"# ipc: mr count=64 mr63=0x");
    let client_said_neg_mr = user::captured_contains(SLOT_CLIENT, b"# ipc: neg mr over=5 ok=1");
    // **KT-290** — ba dòng do **chính chương trình U-mode** in sau `Tcb.Configure` invocation.
    let client_said_cfg = user::captured_contains(SLOT_CLIENT, b"# ipc: configure ok va=0x");
    let client_said_cfg_err = user::captured_contains(SLOT_CLIENT, b"# ipc: configure err=0");
    let client_said_cfg_neg = user::captured_contains(
        SLOT_CLIENT,
        b"# ipc: configure neg cap=18 method=18 outside=5 ok=1",
    );
    // **KT-309** — bốn dòng do **chính hai chương trình U-mode** in (client gửi, server nhận+dùng).
    let client_said_cap = user::captured_contains(SLOT_CLIENT, b"# ipc: cap sent=1 slot=");
    let client_said_cap_neg =
        user::captured_contains(SLOT_CLIENT, b"# ipc: cap neg grant=3 over=5 ok=1");
    let server_said_cap_busy = user::captured_contains(SLOT_SERVER, b"# ipc: cap busy=7 ok=1");
    let server_said_cap_used =
        user::captured_contains(SLOT_SERVER, b"# ipc: cap used slot=5 copy=6");
    // **KT-311** — hai dòng do **chính chương trình U-mode** in sau `Tcb.Configure` kênh đầy đủ.
    let client_said_cfg2 = user::captured_contains(SLOT_CLIENT, b"# ipc: configure2 ok=1 entry=0x");
    // **KT-320** — `vspace=2` (không còn `18`): `vspace?` **đã cài** nên tiền đề "chưa cài ⇒
    // `NotSupported`" của KT-311 **hết hiệu lực** (`k6-test-invariants.md` §4.1); ca âm của
    // KT-311 nay đo **sai loại** (ô 0 của CSpace gốc = cap `CNode` ⇒ `WrongObjectType` 2).
    let client_said_cfg2_neg = user::captured_contains(
        SLOT_CLIENT,
        b"# ipc: configure2 neg align=6 prio=5 cspace=2 vspace=2 ok=1",
    );
    // **KT-314** — hai dòng do **chính chương trình U-mode** in sau `Tcb.Resume`, và dấu của
    // **tác vụ đích** (nó chạy `resume_entry`, không phải `_start`).
    let client_said_resume = user::captured_contains(SLOT_CLIENT, b"# ipc: resume ok=1 entry=0x");
    let client_said_resume_neg = user::captured_contains(
        SLOT_CLIENT,
        b"# ipc: resume neg noentry=5 running=17 exited=17 ok=1",
    );
    let target_said_ran = user::captured_contains(SLOT_CLIENT, b"# ipc: resume ran entry=0x");
    // **KT-320** — hai dòng do **chính chương trình U-mode** in sau `Tcb.Configure` tham số
    // `vspace?`: đường xanh (`vspace` = cap `VSpace` ⇒ `(0, 0)`) và hai mã ca âm (mask có bit lạ
    // ⇒ `InvalidArgument` **5**, CPTR là cap `Frame` ⇒ `WrongObjectType` **2**).
    let client_said_cfg3 = user::captured_contains(SLOT_CLIENT, b"# ipc: configure3 ok=1");
    let client_said_cfg3_neg =
        user::captured_contains(SLOT_CLIENT, b"# ipc: configure3 neg mask=5 frame=2 ok=1");
    // **KT-319** — hai dòng do **chính chương trình U-mode** in sau `Tcb.Suspend`.
    let client_said_suspend = user::captured_contains(SLOT_CLIENT, b"# ipc: suspend ok=1 entry=0x");
    let client_said_suspend_neg = user::captured_contains(
        SLOT_CLIENT,
        b"# ipc: suspend neg arity=5 arity2=5 inactive=17 blocked=17 ro=3 ok=1",
    );
    let server_said_ran = user::captured_contains(SLOT_SERVER, b"# ipc: server ran nr=1");
    let server_said_ran2 = user::captured_contains(SLOT_SERVER, b"# ipc: server ran nr=2");
    // **KT-325** — hai dòng của đường **xoá** + hai TCB/hai VSpace: dòng của client (U-mode) sau
    // `Configure` xoá, và dấu của TCB thứ ba (nó chạy `vspace_clear_entry` **sau khi** `vspace`
    // đã bị xoá — nếu arch còn áp VSpace A thì bản ghi `applied` sẽ khác `None`, xem TAP 10).
    let client_said_vspace2 =
        user::captured_contains(SLOT_CLIENT, b"# ipc: vspace2 set=0 clear=0 entry=0x");
    let clear_said_ran = user::captured_contains(SLOT_CLIENT, b"# ipc: vspace clear ran=1");
    let reply_clean = reply_caller.is_none()
        && core_bridge::with_machine(|m| m.reply_caller(rig.reply).ok().flatten()).is_none();
    let client_exit = match stop9 {
        Some(Stop::Exit { code }) => Some(code),
        _ => None,
    };
    let server_exit = match stop7 {
        Some(Stop::Exit { code }) => Some(code),
        _ => None,
    };
    // **KT-314** — tác vụ đích (lượt 10) phải `Exit(0)` — nó chạy được **chỉ vì** `sepc` đã được
    // nạp từ `TcbData.entry`.
    let target_exit = match stop10 {
        Some(Stop::Exit { code }) => Some(code),
        _ => None,
    };
    // **KT-325** — (a) TCB thứ ba chạy **sau khi** `vspace` bị xoá: phải `Exit(0)` (chạy bằng
    // VSpace của ảnh đã nạp, **không** bị fail-closed) và **không** có bản ghi "đã áp" nào;
    // (b) TCB thứ hai (VSpace B, root khác) bị **từ chối** vào U-mode ⇒ `Stop::Blocked`.
    let clear_exit = match stop_clear {
        Some(Stop::Exit { code }) => Some(code),
        _ => None,
    };
    let clear_refused = matches!(stop_clear, Some(Stop::Blocked(_)));
    let vs2_refused = matches!(stop_vs2, Some(Stop::Blocked(_)));
    // **KT-282/KT-290** — liên kết IPC buffer của client: **KT-290** binding này do chính
    // chương trình U-mode ghi qua `Tcb.Configure` invocation (đo ở lượt 1, không đổi sau đó).
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc buf: bound={} va=0x{:x} mr_off={} mr_words={}",
            u8::from(bound),
            binding.map(|b| b.va).unwrap_or(0),
            vifast_core::IPC_BUFFER_MR_OFFSET,
            vifast_core::IPC_BUFFER_MR_WORDS
        );
    }
    let mut fails = 0usize;
    // TAP 1 — `Call` U-mode **chặn** đúng, message tới server U-mode, caller `BlockedOnReply`.
    let ok1 = blocked_send
        && ep_len == Some(1)
        && ep_mr == Some(ARG)
        && srv_label == LABEL_REQ
        && srv_mr == ARG
        && blocked_reply
        && server_said_ran;
    if ok1 {
        PLATFORM.put_str("ok 1 - ipc: umode call\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 1 - ipc: umode call: blocked_send={} ep_len={} ep_mr=0x{:x} srv_label=0x{:x} srv_mr=0x{:x} blocked_reply={} server_ran={}",
            u8::from(blocked_send),
            ep_len.map(|n| n as u64).unwrap_or(u64::MAX),
            ep_mr.unwrap_or(0),
            srv_label,
            srv_mr,
            u8::from(blocked_reply),
            u8::from(server_said_ran)
        );
        fails += 1;
    }
    // TAP 2 — server `Reply` ⇒ client **resume** với message trả lời; ca âm đúng mã.
    let ok2 = applied
        && client_exit == Some(0)
        && server_exit == Some(0)
        && client_said_call
        && client_said_neg
        && cli_label == LABEL_REPLY
        && cli_mr0 == REPLY_VALUE
        && cli_mr1 == 1
        && reply_clean
        && check_ipc
        && check_caps;
    if ok2 {
        PLATFORM.put_str("ok 2 - ipc: server replied\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 2 - ipc: server replied: applied={} client_exit={} server_exit={} said_call={} said_neg={} reply_label=0x{:x} reply_mr0=0x{:x} reply_mr1={} reply_clean={} check_ipc={} check_caps={}",
            u8::from(applied),
            client_exit.map(|c| c as i64).unwrap_or(-1),
            server_exit.map(|c| c as i64).unwrap_or(-1),
            u8::from(client_said_call),
            u8::from(client_said_neg),
            cli_label,
            cli_mr0,
            cli_mr1,
            u8::from(reply_clean),
            u8::from(check_ipc),
            u8::from(check_caps)
        );
        fails += 1;
    }
    // TAP 3 — **KT-282**: MR4..MR7 (vòng 8 MR) và MR63 (vòng 64 MR) đi qua **IPC buffer** cả
    // hai chiều; kernel đọc **độc lập** `tcb_ipc_area` của cả hai bên, chương trình U-mode đọc
    // buffer của chính nó. `length = 65` ⇒ 5 (không tràn).
    let ok3 = bound
        && srv1 == Some((8, REQ_MR4, REQ_MR7))
        && cli1 == Some((8, REPLY_MR4, REPLY_MR7))
        && srv2 == Some((64, REQ_MR4, REQ_MR63))
        && cli2 == Some((64, REPLY_MR4, REPLY_MR63))
        && client_said_mr8
        && client_said_mr64
        && client_said_neg_mr
        && server_said_ran2
        && applied_s
        && applied2
        && client_exit == Some(0)
        && server_exit == Some(0);
    if ok3 {
        PLATFORM.put_str("ok 3 - ipc: mr4..mr7 via buffer\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 3 - ipc: mr4..mr7 via buffer: bound={} srv1={:?} cli1={:?} srv2={:?} cli2={:?} said_mr8={} said_mr64={} said_neg_mr={} srv_ran2={} applied_s={} applied2={} client_exit={} server_exit={}",
            u8::from(bound),
            srv1,
            cli1,
            srv2,
            cli2,
            u8::from(client_said_mr8),
            u8::from(client_said_mr64),
            u8::from(client_said_neg_mr),
            u8::from(server_said_ran2),
            u8::from(applied_s),
            u8::from(applied2),
            client_exit.map(|c| c as i64).unwrap_or(-1),
            server_exit.map(|c| c as i64).unwrap_or(-1)
        );
        fails += 1;
    }
    // TAP 4 — **KT-290**: `Tcb.Configure` qua **invocation** từ U-mode. Đo **độc lập** ở kernel:
    // `delta.configure == 3` (bind đúng + bind ngoài frame + bind lại), `delta.denied == 2` (cap
    // không phải TCB + method sai), `sys_err(0) == 0` (lỗi `Call` cuối cùng đã trả về U-mode),
    // binding **thật** của TCB client đúng VA (do chính invocation ghi — rig không bind hộ), và
    // ba dòng do chương trình U-mode in. Vòng 8 MR/64 MR của TAP 3 chỉ xanh **sau** khi buffer
    // được bind bằng đường này ⇒ TAP 4 không thể xanh nếu invocation không chạy.
    let ok4 = client_said_cfg
        && client_said_cfg_err
        && client_said_cfg_neg
        && cfg_delta == 3
        && denied_delta == 2
        && cfg_err == 0
        && binding.map(|b| b.va) == Some(vifast_plat_riscv64::ipc_buffer::USER_IPC_BUFFER_VA)
        && client_exit == Some(0)
        && check_ipc
        && check_caps;
    if ok4 {
        PLATFORM.put_str("ok 4 - ipc: tcb configure via invocation\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 4 - ipc: tcb configure via invocation: said_cfg={} said_err={} said_neg={} delta={} denied={} sys_err={} bound={} va=0x{:x} client_exit={} check_ipc={} check_caps={}",
            u8::from(client_said_cfg),
            u8::from(client_said_cfg_err),
            u8::from(client_said_cfg_neg),
            cfg_delta,
            denied_delta,
            cfg_err,
            u8::from(bound),
            binding.map(|b| b.va).unwrap_or(0),
            client_exit.map(|c| c as i64).unwrap_or(-1),
            u8::from(check_ipc),
            u8::from(check_caps)
        );
        fails += 1;
    }
    // TAP 5 — **KT-309**: **cap transfer qua IPC buffer**. Client khai `n = 1` + CPTR trong
    // **vùng dự phòng** của IPC buffer (offset 560); kernel kiểm `RIGHT_GRANT` của cap nguồn rồi
    // **copy** cap sang CSpace server ở slot server khai (`dst`). Đo **độc lập** ở kernel:
    // `extra_caps_sent == 1`, `extra_caps_installed == 1`, cap trong CSpace server là `Frame`
    // **sống** và **đã bỏ `RIGHT_GRANT`**, bản `CNode.Copy` trỏ **cùng** object (server **dùng**
    // được cap — không phải "cap giả"), cộng bốn dòng do hai chương trình U-mode in.
    let ok5 = cap_sent == 1
        && cap_recv == 1
        && matches!(srv_cap, Some((_, ObjType::Frame, rights, true)) if rights & RIGHT_GRANT == 0)
        && matches!(
            (srv_cap, srv_copy),
            (Some(a), Some(b)) if a.0 == b.0 && a.1 == ObjType::Frame && b.3
        )
        && client_said_cap
        && client_said_cap_neg
        && server_said_cap_busy
        && server_said_cap_used
        && applied3
        && client_exit == Some(0)
        && server_exit == Some(0)
        && check_ipc
        && check_caps;
    if ok5 {
        PLATFORM.put_str("ok 5 - ipc: cap transfer\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 5 - ipc: cap transfer: sent={} received={} srv_cap={:?} srv_copy={:?} said_cap={} said_neg={} said_busy={} said_used={} applied3={} client_exit={} server_exit={} check_ipc={} check_caps={}",
            cap_sent,
            cap_recv,
            srv_cap,
            srv_copy,
            u8::from(client_said_cap),
            u8::from(client_said_cap_neg),
            u8::from(server_said_cap_busy),
            u8::from(server_said_cap_used),
            u8::from(applied3),
            client_exit.map(|c| c as i64).unwrap_or(-1),
            server_exit.map(|c| c as i64).unwrap_or(-1),
            u8::from(check_ipc),
            u8::from(check_caps)
        );
        fails += 1;
    }
    {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "# ipc final: client_exit={} server_exit={} target_exit={} reply_clean={} check_ipc={} check_caps={} capture_client={} capture_server={} client={} server={} target={}",
            client_exit.map(|c| c as i64).unwrap_or(-1),
            server_exit.map(|c| c as i64).unwrap_or(-1),
            target_exit.map(|c| c as i64).unwrap_or(-1),
            u8::from(reply_clean),
            u8::from(check_ipc),
            u8::from(check_caps),
            u8::from(client_said_call),
            u8::from(server_said_ran),
            state_c9.map(tcb_state_name).unwrap_or("none"),
            state_s7.map(tcb_state_name).unwrap_or("none"),
            state_t10.map(tcb_state_name).unwrap_or("none")
        );
    }
    // TAP 6 — **KT-311**: `Tcb.Configure` **kênh đầy đủ** (`length = 3`, `MR2` = mask). Đo
    // **độc lập** ở kernel: `entry` = entry của ELF client (đọc từ ảnh, không phải hằng chép
    // tay), `priority` = 4, `cspace` = **CSpace gốc** (client đã trả về sau khi đổi sang
    // `cnode_alt`), `calls = 2` (đường xanh + trả `cspace`), `denied = 4` (bốn ca âm). Cộng hai
    // dòng do chính chương trình U-mode in (đường xanh + bốn mã ca âm).
    let ok6 = client_said_cfg2
        && client_said_cfg2_neg
        && cfg2_entry == Some(client.entry)
        && cfg2_prio == Some(4)
        && cfg2_cspace == Some(rig.cnode_c)
        && cfg2_calls == 2
        && cfg2_denied == 5
        && client_exit == Some(0)
        && check_ipc
        && check_caps;
    if ok6 {
        PLATFORM.put_str("ok 6 - ipc: tcb configure full\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 6 - ipc: tcb configure full: said_ok={} said_neg={} entry=0x{:x} want_entry=0x{:x} prio={} cspace={} want_cspace={} calls={} denied={} client_exit={} check_ipc={} check_caps={}",
            u8::from(client_said_cfg2),
            u8::from(client_said_cfg2_neg),
            cfg2_entry.unwrap_or(0),
            client.entry,
            cfg2_prio.map(|p| p as u64).unwrap_or(u64::MAX),
            cfg2_cspace.map(|c| c as u64).unwrap_or(u64::MAX),
            rig.cnode_c,
            cfg2_calls,
            cfg2_denied,
            client_exit.map(|c| c as i64).unwrap_or(-1),
            u8::from(check_ipc),
            u8::from(check_caps)
        );
        fails += 1;
    }
    // TAP 7 — **KT-314**: `Tcb.Resume` (method 12) **tiêu thụ** `TcbData.entry`. Đo **độc lập** ở
    // kernel: `entry` **thật** của TCB đích (client ghi bằng `Configure(entry?)`), trạng thái
    // **thật** sau `Resume` (`Running`), ca âm `entry = 0` ⇒ `InvalidArgument` 5 với trạng thái
    // **không** đổi, và **tác vụ đích chạy thật** ở `resume_entry` (in dấu + `Exit(0)`) với
    // `tf.sepc = 0` — chỉ hook arch nạp được `sepc` từ `TcbData.entry`. Cộng hai dòng do chính
    // chương trình U-mode in (đường xanh + ba mã ca âm).
    let ok7 = client_said_resume
        && client_said_resume_neg
        && target_said_ran
        && resume_entry.is_some_and(|e| e != 0)
        && resume_state == Some(TcbState::Running { hart: 0 })
        && neg_noentry == Some(KError::InvalidArgument.id())
        && neg_state == Some(TcbState::Inactive)
        && target_exit == Some(0)
        && check_ipc
        && check_caps;
    if ok7 {
        PLATFORM.put_str("ok 7 - ipc: tcb resume\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 7 - ipc: tcb resume: said_ok={} said_neg={} said_ran={} entry=0x{:x} state={} neg_noentry={} neg_state={} target_exit={} check_ipc={} check_caps={}",
            u8::from(client_said_resume),
            u8::from(client_said_resume_neg),
            u8::from(target_said_ran),
            resume_entry.unwrap_or(0),
            resume_state.map(tcb_state_name).unwrap_or("none"),
            neg_noentry.map(u64::from).unwrap_or(u64::MAX),
            neg_state.map(tcb_state_name).unwrap_or("none"),
            target_exit.map(|c| c as i64).unwrap_or(-1),
            u8::from(check_ipc),
            u8::from(check_caps)
        );
        fails += 1;
    }
    let _ = (state_c4, state_s5);
    // TAP 8 — **KT-320**: `Tcb.Configure` tham số `vspace?` (mask bit 1) — **danh tính** `VSpace`
    // vào `TcbData.vspace` rồi arch **tiêu thụ** khi vào U-mode (D2-83: core **không** ghi `satp`).
    // Bốn điều kiện, mỗi điều đo **độc lập**:
    //   1. client (U-mode) in đường xanh + **hai** mã ca âm (`mask=5`, `frame=2`);
    //   2. `TcbData.vspace` đọc ở **kernel** **bằng đúng** object đại diện (client ghi, không tin
    //      lời chương trình);
    //   3. `user_run::vspace_applied` báo arch đã **tiêu thụ** đúng object đó với `match=1`;
    //   4. tác vụ đích vẫn `Exit(0)` — nó chạy được **sau khi** `satp` đổi sang VSpace của TCB.
    let vspace_ok = matches!(
        vspace_applied,
        Some((obj, _, _, true)) if obj == rig.vspace_obj
    );
    let ok8 = client_said_cfg3
        && client_said_cfg3_neg
        && cfg_vspace == Some(rig.vspace_obj)
        && vspace_ok
        && target_exit == Some(0)
        && check_ipc
        && check_caps;
    if ok8 {
        PLATFORM.put_str("ok 8 - ipc: tcb configure vspace\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 8 - ipc: tcb configure vspace: said_ok={} said_neg={} vspace={} want_vspace={} applied={} match={} target_exit={} check_ipc={} check_caps={}",
            u8::from(client_said_cfg3),
            u8::from(client_said_cfg3_neg),
            cfg_vspace.map(u64::from).unwrap_or(u64::MAX),
            rig.vspace_obj,
            vspace_applied.map(|(o, _, _, _)| u64::from(o)).unwrap_or(0),
            vspace_applied.map(|(_, _, _, k)| u8::from(k)).unwrap_or(0),
            target_exit.map(|c| c as i64).unwrap_or(-1),
            u8::from(check_ipc),
            u8::from(check_caps)
        );
        fails += 1;
    }

    // TAP 9 — **KT-319**: `Tcb.Suspend` (method 3) chuyển `Running`/`Ready` → `Inactive`. Đo
    // **độc lập** ở kernel: (a) `Tcb.ReadState` của đích thứ ba **sau** `Suspend` từ U-mode =
    // `Inactive` + `suspended = 0` + `blocked_on_kind = None` + `exit_code = None`; (b) **hai**
    // `Tcb.Suspend` thành công trong lượt 9 (`delta.suspend = 2`, không phải "ít nhất 1"); (c) cặp
    // `Suspend`/`Resume` trên đích `Running` cho lại `Running` và tác vụ **chạy lại thật** ở
    // `resume_entry` (`target_exit = 0` — bằng chứng `TcbData.entry` **giữ** qua `Suspend`); cộng
    // hai dòng do chính chương trình U-mode in (đường xanh + năm mã ca âm).
    let ok9 = client_said_suspend
        && client_said_suspend_neg
        && suspend_calls == 2
        && suspend_read_t3.is_some_and(|s| {
            s.state == TcbState::Inactive
                && !s.suspended
                && s.blocked_on_kind.is_none()
                && s.exit_code.is_none()
        })
        && suspend_state_t3 == Some(TcbState::Inactive)
        && matches!(suspend_state_t4, Some(TcbState::BlockedOnSend { .. }))
        && resume_state == Some(TcbState::Running { hart: 0 })
        && target_said_ran
        && target_exit == Some(0)
        && check_ipc
        && check_caps;
    if ok9 {
        PLATFORM.put_str("ok 9 - ipc: tcb suspend\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 9 - ipc: tcb suspend: said_ok={} said_neg={} calls={} state={} state2={} state4={} suspended={} blocked={} exit={} resumed={} ran={} target_exit={} check_ipc={} check_caps={}",
            u8::from(client_said_suspend),
            u8::from(client_said_suspend_neg),
            suspend_calls,
            suspend_read_t3
                .map(|s| tcb_state_name(s.state))
                .unwrap_or("none"),
            suspend_state_t3.map(tcb_state_name).unwrap_or("none"),
            suspend_state_t4.map(tcb_state_name).unwrap_or("none"),
            suspend_read_t3
                .map(|s| u8::from(s.suspended))
                .unwrap_or(u8::MAX),
            suspend_read_t3
                .map(|s| u8::from(s.blocked_on_kind.is_some()))
                .unwrap_or(u8::MAX),
            suspend_read_t3
                .map(|s| s.exit_code.is_none() as u8)
                .unwrap_or(u8::MAX),
            resume_state.map(tcb_state_name).unwrap_or("none"),
            u8::from(target_said_ran),
            target_exit.map(|c| c as i64).unwrap_or(-1),
            u8::from(check_ipc),
            u8::from(check_caps)
        );
        fails += 1;
    }
    // TAP 10 — **KT-325**: (a) **đường xoá** `vspace?` (CPTR rỗng) và (b) **hai TCB — hai VSpace**
    // ⇒ `vspace_applied` đọc **theo từng TCB**, không đè nhau (nợ §9 của KT-320 đã đóng).
    //
    // Bốn nhóm điều kiện, mỗi nhóm đo **độc lập ở kernel** (không tin lời chương trình):
    //   1. `TcbData.vspace`: T1 = A, T2 = B, T0 = **0** sau khi **xoá**; `entry` của T0 **giữ**
    //      nguyên qua đường xoá (chỉ `vspace` bị xoá);
    //   2. `vspace_applied(T1)` = `(A, match=1)` và `vspace_applied(T2)` = `(B, match=0)` —
    //      **khác nhau** (đột biến "trả `vspace_applied` toàn cục" ⇒ hai TCB **cùng** một giá trị
    //      ⇒ đỏ); T2 bị **từ chối** vào U-mode (`Stop::Blocked`, root khác ảnh đã nạp);
    //   3. `vspace_applied(T0) == None` **sau khi** T0 chạy thật ⇒ arch **không** áp VSpace cũ nữa
    //      (đột biến "bỏ nhánh xoá" ⇒ `Some(A)` ⇒ đỏ); T0 `Exit(0)` — chạy bằng VSpace của ảnh
    //      đã nạp, **không** fail-closed;
    //   4. hai dòng do **chính chương trình U-mode** in (đường xoá + dấu của T0).
    let ok10 = cfg_vspace == Some(rig.vspace_obj)
        && cfg_vspace2 == Some(rig.vspace_obj2)
        && cfg_vspace_cleared == Some(0)
        && entry_cleared.is_some_and(|e| e != 0)
        && matches!(applied_t1_end, Some((obj, _, _, true)) if obj == rig.vspace_obj)
        && matches!(applied_vs2, Some((obj, _, _, false)) if obj == rig.vspace_obj2)
        && applied_t1_end.map(|(o, _, _, _)| o) != applied_vs2.map(|(o, _, _, _)| o)
        && applied_cleared.is_none()
        && !clear_refused
        && clear_exit == Some(0)
        && vs2_refused
        && client_said_vspace2
        && clear_said_ran
        && check_ipc2
        && check_caps2;
    if ok10 {
        PLATFORM.put_str("ok 10 - ipc: tcb configure vspace clear + hai tcb\n");
    } else {
        let mut w = PlatformWriter(&PLATFORM);
        let _ = writeln!(
            w,
            "not ok 10 - ipc: tcb configure vspace clear + hai tcb: v1={} v2={} v0={} entry0=0x{:x} ap1={:?} ap1_luc_ap={:?} ap2={:?} ap0={:?} clear_refused={} clear_exit={} vs2_refused={} said_vs2={} said_ran={} check_ipc={} check_caps={}",
            cfg_vspace.map(u64::from).unwrap_or(u64::MAX),
            cfg_vspace2.map(u64::from).unwrap_or(u64::MAX),
            cfg_vspace_cleared.map(u64::from).unwrap_or(u64::MAX),
            entry_cleared.unwrap_or(0),
            applied_t1_end,
            vspace_applied,
            applied_vs2,
            applied_cleared,
            u8::from(clear_refused),
            clear_exit.map(|c| c as i64).unwrap_or(-1),
            u8::from(vs2_refused),
            u8::from(client_said_vspace2),
            u8::from(clear_said_ran),
            u8::from(check_ipc2),
            u8::from(check_caps2)
        );
        fails += 1;
    }
    // TAP 11 — **KT-329**: điều kiện + chẩn đoán nằm ở `rig::DeferTap` (xem `run_defer_tap`).
    if defer_tap.ok() {
        PLATFORM.put_str("ok 11 - ipc: tcb suspend defer + resume\n");
    } else {
        defer_tap.print_fail();
        fails += 1;
    }
    if fails == 0 {
        PLATFORM.put_str(
            "PANIC: KT-329 ipc u-mode + mr buffer + tcb configure + cap transfer + configure full + tcb resume + configure vspace + tcb suspend + vspace clear + hai tcb + suspend defer xong (11 TAP, 0 fail)\n",
        );
        PLATFORM.halt(0)
    } else {
        PLATFORM.put_str(
            "PANIC: KT-329 ipc u-mode + mr buffer + tcb configure + cap transfer + configure full + tcb resume + configure vspace + tcb suspend + vspace clear + hai tcb + suspend defer CO LOI\n",
        );
        PLATFORM.halt(1)
    }
}

/// Tên loại object cho log (chỉ những loại ảnh này dùng).
fn obj_type_name(ty: ObjType) -> &'static str {
    match ty {
        ObjType::Frame => "Frame",
        ObjType::CNode => "CNode",
        ObjType::Tcb => "Tcb",
        ObjType::Endpoint => "Endpoint",
        ObjType::Reply => "Reply",
        ObjType::VSpace => "VSpace",
        ObjType::Untyped => "Untyped",
        ObjType::Notification => "Notification",
        ObjType::SchedContext => "SchedContext",
        ObjType::SchedControl => "SchedControl",
        _ => "other",
    }
}

/// Tên trạng thái TCB cho log (cùng bảng với `ktest_user`/`ktest_proc`).
fn tcb_state_name(state: TcbState) -> &'static str {
    match state {
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

/// Tên `Stop` cho log.
fn stop_name(stop: Option<Stop>) -> &'static str {
    match stop {
        Some(Stop::Exit { .. }) => "Exit",
        Some(Stop::Yield) => "Yield",
        Some(Stop::Blocked(_)) => "Blocked",
        Some(Stop::Halt { .. }) => "Halt",
        Some(Stop::Fault) => "Fault",
        None => "None",
    }
}

/// Entry hart **thứ cấp** (matrix `-smp 2/4`): adopt `satp`/`stvec` của BSP rồi park — KT-280/
/// KT-282 không cần hart phụ (hai tác vụ U-mode trên một hart), nhưng linker đòi symbol cho mọi ảnh.
#[no_mangle]
pub extern "C" fn rust_secondary_main(_hart: usize, opaque: usize) -> ! {
    let _ = vifast_plat_riscv64::smp::secondary_setup(opaque);
    vifast_plat_riscv64::park_forever()
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    PANIC.handle(info)
}
