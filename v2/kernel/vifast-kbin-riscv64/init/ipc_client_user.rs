// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-280** — `ipc_client` U-mode: gọi `Call` **chặn** tới server U-mode rồi nhận reply.
//!
//! **KT-282** — mở rộng: hai vòng `Call` mang **8 MR** rồi **64 MR**, trong đó **MR4..MR63 đi
//! qua IPC buffer** (offset 80 B, `[TK]` KT-282) — MR0..MR3 vẫn ở thanh ghi `a3..a6`.
//!
//! **KT-290** — **`Tcb.Configure` qua invocation từ U-mode**: trước KT-290 rig gọi thẳng API core
//! (`tcb_configure_with_ipc_buffer`) để bind IPC buffer; nay **chính chương trình U-mode** gọi
//! `Call` trên cap **TCB** với `label = TCB_CONFIGURE` (0, hằng **sinh từ spec**) và
//! `length = 2` (`MR0` = CPTR cap `Frame`, `MR1` = VA) ⇒ kernel bind buffer rồi trả
//! `(error, value) = (0, 0)`. Đây là mắt xích còn thiếu của KT-282 §6.3: nếu invocation không
//! chạy, vòng 8 MR/64 MR **không thể** xanh (buffer chưa bind ⇒ `InvalidArgument` 5).
//!
//! **KT-311** — **`Tcb.Configure` kênh đầy đủ**: `length = 3` + `MR2` = **mask** tham số mở rộng;
//! `cspace?`/`vspace?`/`entry?`/`priority?` nằm trong **vùng riêng** của IPC buffer (byte
//! 608..640, `[TK]` KT-311 — **không** đổi ABI). Bốn ca âm (entry lẻ ⇒ 6, priority 256 ⇒ 5,
//! cspace = cap `Frame` ⇒ 2, vspace ⇒ 18), rồi đường xanh (`cspace` = `cnode_alt`, `entry` =
//! `_start` thật, `priority` = 4) + **chứng minh** `cspace?` thật sự đổi CSpace (`Call` trên ô 8
//! **chỉ có** ở `cnode_alt` ⇒ 18 thay vì 1) + trả `cspace` về CSpace gốc.
//!
//! Đây là mắt xích mà K3 chưa có: trước KT-280 mọi `call()` của chương trình U-mode đều nhắm cap
//! **không phải Endpoint** (invocation `Frame.Map`/`Untyped.Retype`/`Tcb.SetTls`/`CNode.*`) nên
//! trả về ngay; IPC U-mode hiện có là `NBSend`+`NBRecv` polling. Ở đây client gọi `Call` trên
//! **Endpoint** ⇒ core đưa TCB sang `BlockedOnReply` ⇒ kernel chạy **server U-mode** ⇒ server
//! `Recv` + `Send` trên cap `Reply` ⇒ client được **resume** với `a0..a6` + **MR4..MR63 trong IPC
//! buffer** lấy từ vùng nhận IPC.
//!
//! Bố cục cspace (kernel dựng, xem `src/ktest_ipc.rs`):
//!
//! | slot | cap |
//! |---|---|
//! | 0 | `CNode` của chính cspace |
//! | 1 | `Endpoint` `EP` (gửi `Call`) |
//! | 2 | `Reply` (client **không** dùng — ca âm "Call trên cap không phải Endpoint") |
//! | 3 | `Tcb` của chính client — **KT-290** `Call` với `label = Tcb.Configure` |
//! | 4 | `Frame` 4 KiB — IPC buffer bind qua invocation (KT-290) |
//! | 5/6 | `Frame` nguồn extra cap (có/thiếu `RIGHT_GRANT`) — KT-309 |
//! | 7 | `CNode` của **CSpace thứ hai** (`cnode_alt`) — **KT-311** `cspace?` |
//! | 8 | ô **rỗng** ở CSpace gốc (nhưng có cap `Reply` ở `cnode_alt`) — KT-311 chứng minh `cspace?` |
//! | 9 | ô **rỗng** — ca âm `Call` trên ô rỗng |
//!
//! Không `unsafe`, không `asm!` (R5): mọi `ecall` qua shim `vifast_user_abi`.
#![no_std]
#![no_main]

extern crate vifast_abi;
extern crate vifast_user_abi;

use vifast_user_abi::{
    call, exit, info, info_label, info_length, init_ipc_buffer, mrs, put, set_configure_params,
    set_extra_caps, set_mrs, set_recv_window,
};

/// Slot `Endpoint` — nơi client gửi `Call`.
const SLOT_EP: u64 = 1;
/// Slot `Reply` — **không** phải Endpoint: `Call` trên nó phải trả lỗi (ca âm).
const SLOT_REPLY: u64 = 2;
/// **KT-290** — slot cap `Tcb` của chính client (kernel dựng): `Call` với `label = Configure`.
const SLOT_TCB: u64 = 3;
/// **KT-290** — slot cap `Frame` 4 KiB làm IPC buffer (kernel dựng).
const SLOT_FRAME: u64 = 4;
/// Slot **rỗng** — `Call` trên ô rỗng phải trả `InvalidCapability` (1) (ca âm).
const SLOT_EMPTY: u64 = 9;

/// **KT-290** — method id của `Tcb.Configure` **sinh từ `spec/abi.toml`** (`[object].Tcb` method
/// 0), không chép số tay (D2-11) — cùng nguồn với core.
const METHOD_CONFIGURE: u32 = vifast_abi::TCB_CONFIGURE;
/// **KT-314** — method id của `Tcb.Resume` **sinh từ `spec/abi.toml`** (`[object].Tcb` method
/// 12), không chép số tay (D2-11) — cùng nguồn với core.
const METHOD_RESUME: u32 = vifast_abi::TCB_RESUME;
/// **KT-290** — VA cố định của trang IPC buffer (loader map `R|W|U|NX` cho mọi ELF U-mode).
const IPC_BUF_VA: u64 = 0x4002_0000;
/// **KT-290** — VA **ngoài** frame IPC buffer (trang kế, chưa map) — ca âm fail-closed.
const IPC_BUF_VA_OUTSIDE: u64 = 0x4002_1000;

/// **KT-309** — slot cap `Frame` **có** `RIGHT_GRANT` (nguồn extra cap đi qua IPC buffer).
const SLOT_FRAME_GRANT: u64 = 5;
/// **KT-309** — slot cap `Frame` **thiếu** `RIGHT_GRANT` — ca âm (i).
const SLOT_FRAME_NO_GRANT: u64 = 6;
/// **KT-311** — slot cap `CNode` của **CSpace thứ hai** (`cnode_alt`) — `cspace?` đường xanh.
const SLOT_ALT_CNODE: u64 = 7;
/// **KT-311** — ô **chỉ có** trong `cnode_alt` (cap `Reply`): `Call` trên nó trả **18** khi
/// CSpace đã đổi, **1** (`InvalidCapability`) khi vẫn ở CSpace gốc — chứng minh `cspace?` **thật
/// sự** đổi CSpace, không phải chỉ ghi một trường.
const SLOT_ALT_ONLY: u64 = 8;
/// **KT-311** — trong `cnode_alt`: ô 0 = cap tới **CSpace gốc** (để trả `cspace` về), ô 3 = cap
/// `Tcb` của chính client (để gọi `Configure` khi CSpace đang là `cnode_alt`).
const SLOT_ALT_BACK: u64 = 0;
const SLOT_ALT_TCB: u64 = 3;
/// **KT-314** — cap `Tcb` của **server** (đã `Exit` ở lượt 7 ⇒ `Inactive` + `exit_code`):
/// `Resume` trên nó phải trả `IllegalOperation` (**17**) — không tự chạy lại tác vụ đã kết thúc.
const SLOT_TCB_S: u64 = 14;
/// **KT-314** — cap `Tcb` **đích** (rig để `Inactive`, chưa từng chạy): `Configure(entry?)` rồi
/// `Resume` ⇒ kernel nạp `sepc` = `entry` ở lượt chạy kế tiếp của nó.
const SLOT_TCB_T: u64 = 15;
/// **KT-314** — cap `Tcb` **đích thứ hai** (`Inactive`, `entry = 0`): `Resume` ⇒
/// `InvalidArgument` (5) — fail-closed, không đoán điểm vào.
const SLOT_TCB_T2: u64 = 16;
/// **KT-311** — mask tham số mở rộng (`MR2` của `Tcb.Configure`, `length = 3`).
const MASK_CSPACE: u64 = vifast_user_abi::CONFIGURE_MASK_CSPACE;
const MASK_VSPACE: u64 = vifast_user_abi::CONFIGURE_MASK_VSPACE;
const MASK_ENTRY: u64 = vifast_user_abi::CONFIGURE_MASK_ENTRY;
const MASK_PRIORITY: u64 = vifast_user_abi::CONFIGURE_MASK_PRIORITY;
/// **KT-311** — VA entry **không căn 2 B** (lẻ — ca âm `Alignment`). VA entry **hợp lệ** là địa
/// chỉ `_start` **thật** của chính ảnh này (`_start as usize` — trùng `e_entry` của ELF, không
/// chép tay hằng số): rig đối chiếu `TcbData.entry` với `LoadedInit.entry` đọc từ ELF.
const ENTRY_VA_BAD: u64 = 0x4000_0001;
/// **KT-311** — priority hợp lệ (≤ `mcp` = 10 của rig) và **ngoài dải** `u8` (ca âm).
const PRIORITY_OK: u64 = 4;
const PRIORITY_BAD: u64 = 256;
/// **KT-309** — label của message mang extra cap (hợp đồng client ↔ server).
const LABEL_CAP: u32 = 0x282;
/// **KT-309** — label reply của server cho message mang extra cap.
const LABEL_CAP_REPLY: u32 = 0x283;

/// Label request (không phải ABI — hợp đồng giữa client và server của bài test).
const LABEL_REQ: u32 = 0x280;
/// Label reply mà server gửi lại.
const LABEL_REPLY: u32 = 0x281;
/// MR0 của request — server phải thấy đúng giá trị này.
const ARG: u64 = 0x1234;
/// MR0 của reply — client phải nhận đúng giá trị này.
const REPLY_VALUE: u64 = 0x5a5a;

/// **KT-282** — MR4..MR7 của request vòng 1 (đi qua **IPC buffer**, không phải thanh ghi).
const REQ_MR4: u64 = 0x2820_0004;
const REQ_MR5: u64 = 0x2820_0005;
const REQ_MR6: u64 = 0x2820_0006;
const REQ_MR7: u64 = 0x2820_0007;
/// **KT-282** — MR63 (word cuối vùng buffer) của request vòng 2 (`length = 64`).
const REQ_MR63: u64 = 0x2820_003f;
/// **KT-282** — MR4..MR7 / MR63 của **reply** (server ghi vào buffer của nó, kernel chép sang).
const REPLY_MR4: u64 = 0x2821_0004;
const REPLY_MR7: u64 = 0x2821_0007;
const REPLY_MR63: u64 = 0x2821_003f;

/// Mã thoát khi hỏng (TAP đỏ vì khác 0).
const EXIT_CALL: u64 = 80;
const EXIT_LABEL: u64 = 81;
const EXIT_VALUE: u64 = 82;
const EXIT_NR: u64 = 83;
const EXIT_NEG_EMPTY: u64 = 84;
const EXIT_NEG_REPLY: u64 = 85;
const EXIT_MR_LEN: u64 = 86;
const EXIT_MR_VALUE: u64 = 87;
const EXIT_MR64_CALL: u64 = 88;
const EXIT_MR64_LEN: u64 = 89;
const EXIT_MR64_VALUE: u64 = 90;
const EXIT_MR_OVER: u64 = 91;
/// **KT-290** — `Tcb.Configure` invocation (bind IPC buffer) hỏng.
const EXIT_CONFIGURE: u64 = 92;
/// **KT-290** — ca âm của `Tcb.Configure` (cap sai loại / method sai) trả mã khác 18.
const EXIT_CONFIGURE_NEG: u64 = 93;
/// **KT-290** — ca âm "buffer ngoài frame" không fail-closed đúng 5.
const EXIT_CONFIGURE_OUTSIDE: u64 = 94;
/// **KT-309** — ca âm (i): cap nguồn thiếu `RIGHT_GRANT` không trả `InsufficientRights` (3).
const EXIT_CAP_GRANT: u64 = 95;
/// **KT-309** — ca âm (ii): `n = 5` (> `max_caps = 4`) không trả `InvalidArgument` (5).
const EXIT_CAP_OVER: u64 = 96;
/// **KT-309** — `Call` mang extra cap không trả 0.
const EXIT_CAP_CALL: u64 = 97;
/// **KT-309** — reply của server cho message mang extra cap sai label/slot.
const EXIT_CAP_REPLY: u64 = 98;
/// **KT-311** — ca âm (i): `entry` không căn 4 B không trả `Alignment` (6).
const EXIT_CFG2_ALIGN: u64 = 100;
/// **KT-311** — ca âm (ii): `priority` ngoài dải không trả `InvalidArgument` (5).
const EXIT_CFG2_PRIO: u64 = 101;
/// **KT-311** — ca âm (iii): `cspace` không phải `CNode` không trả `WrongObjectType` (2).
const EXIT_CFG2_CSPACE: u64 = 102;
/// **KT-311** — ca âm (iv): `vspace?` (chưa cài) không trả `NotSupported` (18).
const EXIT_CFG2_VSPACE: u64 = 103;
/// **KT-311** — `Tcb.Configure` kênh đầy đủ (mask = cspace|entry|priority) không trả 0.
const EXIT_CFG2_FULL: u64 = 104;
/// **KT-311** — `cspace?` không **thật sự** đổi CSpace (ô 8 vẫn trả 1 thay vì 18).
const EXIT_CFG2_MOVED: u64 = 105;
/// **KT-311** — trả `cspace` về CSpace gốc hỏng, hoặc ô 9 không trả 1 (chưa về CSpace gốc).
const EXIT_CFG2_BACK: u64 = 106;
/// **KT-314** — `Resume` trên TCB `entry = 0` **không** trả `InvalidArgument` (5).
const EXIT_RESUME_NOENTRY: u64 = 107;
/// **KT-314** — `Resume` trên tác vụ **`Running`** (chính ta) **không** trả `IllegalOperation`
/// (**17**) — hoặc kiểm `entry` chạy **trước** kiểm trạng thái (sẽ ra 5).
const EXIT_RESUME_RUNNING: u64 = 108;
/// **KT-314** — `Resume` trên tác vụ **đã `Exit`** (`Inactive` + `exit_code`) không trả **17**.
const EXIT_RESUME_EXITED: u64 = 109;
/// **KT-314** — đường xanh `Configure(entry?)` + `Resume` không trả `(0, entry)`.
const EXIT_RESUME_FULL: u64 = 110;

/// In số thập phân (không cấp phát, không `unsafe`) — cùng khuôn `con_client_user.rs`.
fn put_u64(mut value: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    if value == 0 {
        i -= 1;
        buf[i] = b'0';
    }
    while value > 0 {
        i -= 1;
        buf[i] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    put(&buf[i..]);
}

/// In số hex thập lục phân **không** tiền tố `0x` (chữ thường) — cho dòng `# ipc: mr …`.
fn put_hex(mut value: u64) {
    let digits = b"0123456789abcdef";
    let mut buf = [0u8; 16];
    let mut i = buf.len();
    if value == 0 {
        i -= 1;
        buf[i] = b'0';
    }
    while value > 0 {
        i -= 1;
        buf[i] = digits[(value & 0xf) as usize];
        value >>= 4;
    }
    put(&buf[i..]);
}

/// **KT-314** — điểm vào **thay thế** của TCB đích: `Tcb.Resume` nạp `sepc` từ `TcbData.entry`
/// ⇒ tác vụ đích bắt đầu chạy **ở đây** (không phải `_start`). In dấu rồi `Exit(0)`.
///
/// Địa chỉ hàm này là giá trị `entry?` mà client ghi vào TCB đích (`resume_entry as usize`);
/// rig **không** đặt `tf.sepc` cho lượt chạy của đích (để `0`) ⇒ nếu `Resume`/hook arch không
/// nạp `sepc` thì đích fault ở `0` và TAP 7 đỏ.
#[no_mangle]
pub extern "C" fn resume_entry() -> ! {
    put(b"# ipc: resume ran entry=0x");
    put_hex(resume_entry as usize as u64);
    put(b"\n");
    exit(0)
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    init_ipc_buffer();
    // Receive window 4 ô **phân biệt** (spec dòng 58): core từ chối window trùng ô
    // (`InvalidArgument` 5) — message của bài test không mang cap nên window chỉ cần hợp lệ.
    // **KT-290**: ô 4 đã là cap `Frame` (IPC buffer) ⇒ dời window sang 5..8 (đều rỗng).
    // **KT-309**: ô 5/6 là cap `Frame` **nguồn** extra cap ⇒ dời window sang 10..13 (đều rỗng).
    set_recv_window([10, 11, 12, 13]);

    // ---- 0) **KT-290** — `Tcb.Configure` qua **invocation** từ U-mode: bind IPC buffer.
    // `Call(cap TCB, MessageInfo{label = TCB_CONFIGURE, length = 2}, MR0 = CPTR Frame,
    // MR1 = VA)`. Kernel kiểm cap/quyền/arity/loại Frame/VA rồi bind; trả `a0 = error`,
    // `a1 = value`. Đây là đường **duy nhất** bind buffer của client trong ảnh này (rig
    // **không** bind hộ) ⇒ vòng 8 MR/64 MR bên dưới chỉ chạy được nếu invocation xanh.
    let cfg = call(
        SLOT_TCB,
        info(METHOD_CONFIGURE, 0, false, 2),
        [SLOT_FRAME, IPC_BUF_VA, 0, 0],
    );
    if cfg[0] != 0 {
        put(b"# ipc: configure err=");
        put_u64(cfg[0]);
        put(b"\n");
        exit(EXIT_CONFIGURE);
    }
    put(b"# ipc: configure ok va=0x");
    put_hex(IPC_BUF_VA);
    put(b"\n");
    put(b"# ipc: configure err=0\n");

    // ---- 0a) ca âm (i): cap **không phải TCB** (Reply) + `label = Configure` ⇒ danh sách
    // trắng hẹp trả `NotSupported` **18** (không phải 0/2) — không có đường "đoán TCB".
    let neg_cap = call(
        SLOT_REPLY,
        info(METHOD_CONFIGURE, 0, false, 2),
        [SLOT_FRAME, IPC_BUF_VA, 0, 0],
    );
    if neg_cap[0] != 18 {
        exit(EXIT_CONFIGURE_NEG);
    }
    // ---- 0b) ca âm (ii): **method id sai** (1 = `SetFaultEp`, chưa cài) trên cap TCB ⇒ 18.
    let neg_method = call(
        SLOT_TCB,
        info(1, 0, false, 2),
        [SLOT_FRAME, IPC_BUF_VA, 0, 0],
    );
    if neg_method[0] != 18 {
        exit(EXIT_CONFIGURE_NEG);
    }
    // ---- 0c) ca âm (iii): **buffer ngoài frame** — bind VA `0x4002_1000` (trang kế, chưa map).
    // Core **không** sở hữu PTE (D2-83) nên `Configure` trả 0 (VA căn 4 KiB, không tràn); lỗi
    // phải lộ ra ở **đường dùng** buffer: `Call` 8 MR ⇒ `InvalidArgument` **5** (arch kiểm dải
    // đã map trước khi đọc — fail-closed, không đọc bừa ngoài frame).
    let outside = call(
        SLOT_TCB,
        info(METHOD_CONFIGURE, 0, false, 2),
        [SLOT_FRAME, IPC_BUF_VA_OUTSIDE, 0, 0],
    );
    if outside[0] != 0 {
        exit(EXIT_CONFIGURE_OUTSIDE);
    }
    let bad = call(SLOT_EP, info(LABEL_REQ, 0, false, 8), [ARG, 0, 0, 0]);
    if bad[0] != 5 {
        exit(EXIT_CONFIGURE_OUTSIDE);
    }
    // Bind lại VA **đúng** để vòng 8 MR thật chạy được (cùng một invocation, lần thứ ba).
    let rebind = call(
        SLOT_TCB,
        info(METHOD_CONFIGURE, 0, false, 2),
        [SLOT_FRAME, IPC_BUF_VA, 0, 0],
    );
    if rebind[0] != 0 {
        exit(EXIT_CONFIGURE);
    }
    put(b"# ipc: configure neg cap=");
    put_u64(neg_cap[0]);
    put(b" method=");
    put_u64(neg_method[0]);
    put(b" outside=");
    put_u64(bad[0]);
    put(b" ok=1\n");

    // ---- 1) `Call` **chặn** trên Endpoint, message **8 MR**: MR0..MR3 ở thanh ghi `a3..a6`,
    //         MR4..MR7 ở **IPC buffer** (offset 80 — KT-282). Kernel chuyển sang server U-mode
    //         rồi resume ta. `answer = [error, value, MessageInfo, MR0..MR3]`.
    set_mrs(&[REQ_MR4, REQ_MR5, REQ_MR6, REQ_MR7]);
    let answer = call(SLOT_EP, info(LABEL_REQ, 0, false, 8), [ARG, 0, 0, 0]);
    if answer[0] != 0 {
        put(b"# ipc: client call err=");
        put_u64(answer[0]);
        put(b"\n");
        exit(EXIT_CALL);
    }
    // `a2` = MessageInfo của **reply** (label + length), `a3` = MR0, `a4` = MR1.
    if info_label(answer[2]) != LABEL_REPLY {
        exit(EXIT_LABEL);
    }
    if info_length(answer[2]) != 8 {
        exit(EXIT_MR_LEN);
    }
    if answer[3] != REPLY_VALUE {
        exit(EXIT_VALUE);
    }
    // MR1 = số request server đã phục vụ (phải là 1 — server chạy đúng một lượt).
    if answer[4] != 1 {
        exit(EXIT_NR);
    }
    // **KT-282** — MR4..MR7 của reply đọc từ **IPC buffer của chính ta** (kernel ghi khi resume).
    let reply_mrs = mrs();
    if reply_mrs[0] != REPLY_MR4 || reply_mrs[3] != REPLY_MR7 {
        exit(EXIT_MR_VALUE);
    }
    put(b"# ipc: umode call ok reply=");
    put_u64(answer[3]);
    put(b"\n");
    put(b"# ipc: mr count=8 mr7=0x");
    put_hex(reply_mrs[3]);
    put(b"\n");

    // ---- 1b) **KT-314** — cấu hình `entry` cho TCB **server** (nó sẽ `Exit` ở lượt 7). Nhờ vậy
    //          ca âm "resume tác vụ đã kết thúc" bị chặn bởi **kiểm trạng thái**, **không** phải
    //          bởi `entry = 0` — đột biến bỏ kiểm trạng thái sẽ **chạy lại** server (đỏ), đúng
    //          điều cần chứng minh. `Configure` **không** đổi trạng thái TCB (D2-25 chỉ xoá
    //          `exit_code`, lúc này còn `None`).
    set_configure_params(0, 0, resume_entry as usize as u64, 0);
    let cfg_s = call(
        SLOT_TCB_S,
        info(METHOD_CONFIGURE, 0, false, 3),
        [SLOT_FRAME, IPC_BUF_VA, MASK_ENTRY, 0],
    );
    if cfg_s[0] != 0 {
        put(b"# ipc: resume cfg server err=");
        put_u64(cfg_s[0]);
        put(b"\n");
        exit(EXIT_RESUME_EXITED);
    }

    // ---- 2) `Call` thứ hai, message **64 MR** (`max_words = 64`): MR0..MR3 thanh ghi +
    //         MR4..MR63 buffer (60 word). Đây là biên trên của vùng buffer — `length = 65` phải
    //         bị từ chối (ca âm (c) dưới).
    let mut words = [0u64; 60];
    words[0] = REQ_MR4;
    words[59] = REQ_MR63;
    set_mrs(&words);
    let answer = call(SLOT_EP, info(LABEL_REQ, 0, false, 64), [ARG, 0, 0, 0]);
    if answer[0] != 0 {
        exit(EXIT_MR64_CALL);
    }
    if info_length(answer[2]) != 64 {
        exit(EXIT_MR64_LEN);
    }
    let reply_mrs = mrs();
    if reply_mrs[0] != REPLY_MR4 || reply_mrs[59] != REPLY_MR63 {
        exit(EXIT_MR64_VALUE);
    }
    put(b"# ipc: mr count=64 mr63=0x");
    put_hex(reply_mrs[59]);
    put(b"\n");

    // ---- 2b) **KT-309** — **cap transfer qua IPC buffer**: gửi **một** cap `Frame` từ client
    //          sang server. `set_extra_caps` ghi `n` + CPTR vào **vùng dự phòng** của IPC buffer
    //          (offset 560, `[TK]` KT-309); kernel đọc vùng đó ở `Call`, kiểm `RIGHT_GRANT` của
    //          cap nguồn, rồi **copy** cap sang CSpace server ở slot server đã khai.
    //
    //          Ca âm (i): cap nguồn **thiếu `RIGHT_GRANT`** ⇒ `InsufficientRights` (3).
    set_extra_caps(&[SLOT_FRAME_NO_GRANT]);
    let neg_grant = call(SLOT_EP, info(LABEL_CAP, 0, false, 1), [ARG, 0, 0, 0]);
    if neg_grant[0] != 3 {
        put(b"# ipc: cap neg grant err=");
        put_u64(neg_grant[0]);
        put(b"\n");
        exit(EXIT_CAP_GRANT);
    }
    //          Ca âm (ii): `n = 5` > `max_caps = 4` ⇒ `InvalidArgument` (5) — biên **không** nới.
    set_extra_caps(&[
        SLOT_FRAME_GRANT,
        SLOT_FRAME_GRANT,
        SLOT_FRAME_GRANT,
        SLOT_FRAME_GRANT,
        SLOT_FRAME_GRANT,
    ]);
    let neg_over = call(SLOT_EP, info(LABEL_CAP, 0, false, 1), [ARG, 0, 0, 0]);
    if neg_over[0] != 5 {
        put(b"# ipc: cap neg over err=");
        put_u64(neg_over[0]);
        put(b"\n");
        exit(EXIT_CAP_OVER);
    }
    //          Đường xanh: **một** cap `Frame` (có `RIGHT_GRANT`) ⇒ server nhận ở slot nó khai.
    set_extra_caps(&[SLOT_FRAME_GRANT]);
    let cap_ans = call(SLOT_EP, info(LABEL_CAP, 0, false, 1), [ARG, 0, 0, 0]);
    if cap_ans[0] != 0 {
        put(b"# ipc: cap call err=");
        put_u64(cap_ans[0]);
        put(b"\n");
        exit(EXIT_CAP_CALL);
    }
    if info_label(cap_ans[2]) != LABEL_CAP_REPLY || cap_ans[3] == 0 {
        exit(EXIT_CAP_REPLY);
    }
    put(b"# ipc: cap sent=1 slot=");
    put_u64(cap_ans[3]);
    put(b"\n");
    put(b"# ipc: cap neg grant=");
    put_u64(neg_grant[0]);
    put(b" over=");
    put_u64(neg_over[0]);
    put(b" ok=1\n");

    // ---- 6) **KT-311** — `Tcb.Configure` **kênh đầy đủ** (`length = 3`, `MR2` = mask): các
    //         tham số `cspace?`/`entry?`/`priority?` nằm trong **vùng riêng** của IPC buffer
    //         (byte 608..640, `[TK]` KT-311) — **không** đổi ABI, **không** đổi `MessageInfo`.
    //         Bốn ca âm trước (mỗi ca đúng **một** tham số sai), rồi đường xanh + chứng minh
    //         `cspace?` **thật sự** đổi CSpace (ô 8 chỉ có trong `cnode_alt`).
    //
    //         Ca âm (i): `entry` **không căn 4 B** ⇒ `Alignment` (6).
    set_configure_params(0, 0, ENTRY_VA_BAD, 0);
    let neg_align = call(
        SLOT_TCB,
        info(METHOD_CONFIGURE, 0, false, 3),
        [SLOT_FRAME, IPC_BUF_VA, MASK_ENTRY, 0],
    );
    if neg_align[0] != 6 {
        put(b"# ipc: configure2 neg align err=");
        put_u64(neg_align[0]);
        put(b"\n");
        exit(EXIT_CFG2_ALIGN);
    }
    //         Ca âm (ii): `priority` **ngoài dải** `u8` (256) ⇒ `InvalidArgument` (5).
    set_configure_params(0, 0, 0, PRIORITY_BAD);
    let neg_prio = call(
        SLOT_TCB,
        info(METHOD_CONFIGURE, 0, false, 3),
        [SLOT_FRAME, IPC_BUF_VA, MASK_PRIORITY, 0],
    );
    if neg_prio[0] != 5 {
        put(b"# ipc: configure2 neg prio err=");
        put_u64(neg_prio[0]);
        put(b"\n");
        exit(EXIT_CFG2_PRIO);
    }
    //         Ca âm (iii): `cspace` = cap `Frame` (ô 4) — **không phải `CNode`** ⇒
    //         `WrongObjectType` (2).
    set_configure_params(SLOT_FRAME, 0, 0, 0);
    let neg_cspace = call(
        SLOT_TCB,
        info(METHOD_CONFIGURE, 0, false, 3),
        [SLOT_FRAME, IPC_BUF_VA, MASK_CSPACE, 0],
    );
    if neg_cspace[0] != 2 {
        put(b"# ipc: configure2 neg cspace err=");
        put_u64(neg_cspace[0]);
        put(b"\n");
        exit(EXIT_CFG2_CSPACE);
    }
    //         Ca âm (iv): `vspace?` **chưa cài** ⇒ `NotSupported` (18) — fail-closed.
    let neg_vspace = call(
        SLOT_TCB,
        info(METHOD_CONFIGURE, 0, false, 3),
        [SLOT_FRAME, IPC_BUF_VA, MASK_VSPACE, 0],
    );
    if neg_vspace[0] != 18 {
        put(b"# ipc: configure2 neg vspace err=");
        put_u64(neg_vspace[0]);
        put(b"\n");
        exit(EXIT_CFG2_VSPACE);
    }
    //         Đường xanh: `cspace` = ô 7 (cap `CNode` của `cnode_alt`), `entry` = 0x4000_0000,
    //         `priority` = 4 (≤ `mcp` = 10). Kernel kiểm **tất cả** rồi mới ghi (nguyên tử).
    let entry_va = _start as usize as u64;
    set_configure_params(SLOT_ALT_CNODE, 0, entry_va, PRIORITY_OK);
    let full = call(
        SLOT_TCB,
        info(METHOD_CONFIGURE, 0, false, 3),
        [
            SLOT_FRAME,
            IPC_BUF_VA,
            MASK_CSPACE | MASK_ENTRY | MASK_PRIORITY,
            0,
        ],
    );
    if full[0] != 0 {
        put(b"# ipc: configure2 full err=");
        put_u64(full[0]);
        put(b"\n");
        exit(EXIT_CFG2_FULL);
    }
    //         Chứng minh `cspace?` **thật sự** đổi CSpace: ô 8 **chỉ có** trong `cnode_alt`
    //         (cap `Reply`) ⇒ `Call` trên nó trả **18** (`NotSupported` — cap sống, không phải
    //         Endpoint); nếu CSpace **không** đổi thì ô 8 rỗng ⇒ **1** (`InvalidCapability`).
    let moved = call(SLOT_ALT_ONLY, info(LABEL_REQ, 0, false, 1), [ARG, 0, 0, 0]);
    if moved[0] != 18 {
        put(b"# ipc: configure2 moved err=");
        put_u64(moved[0]);
        put(b"\n");
        exit(EXIT_CFG2_MOVED);
    }
    //         Trả `cspace` về CSpace gốc: trong `cnode_alt`, ô 0 = cap tới CSpace gốc.
    set_configure_params(SLOT_ALT_BACK, 0, 0, 0);
    let back = call(
        SLOT_ALT_TCB,
        info(METHOD_CONFIGURE, 0, false, 3),
        [SLOT_FRAME, IPC_BUF_VA, MASK_CSPACE, 0],
    );
    if back[0] != 0 {
        put(b"# ipc: configure2 back err=");
        put_u64(back[0]);
        put(b"\n");
        exit(EXIT_CFG2_BACK);
    }
    //         Chứng minh đã về CSpace gốc: ô 9 **rỗng** ở đó ⇒ `Call` trả **1**.
    let restored = call(SLOT_EMPTY, info(LABEL_REQ, 0, false, 1), [ARG, 0, 0, 0]);
    if restored[0] != 1 {
        put(b"# ipc: configure2 restored err=");
        put_u64(restored[0]);
        put(b"\n");
        exit(EXIT_CFG2_BACK);
    }
    put(b"# ipc: configure2 ok=1 entry=0x");
    put_hex(entry_va);
    put(b" prio=");
    put_u64(PRIORITY_OK);
    put(b"\n");
    put(b"# ipc: configure2 neg align=");
    put_u64(neg_align[0]);
    put(b" prio=");
    put_u64(neg_prio[0]);
    put(b" cspace=");
    put_u64(neg_cspace[0]);
    put(b" vspace=");
    put_u64(neg_vspace[0]);
    put(b" ok=1\n");

    // ---- 7) **Nhường CPU** — ranh giới để kernel đo **độc lập** `TcbData.entry`/`priority`/
    //         `cspace` **ngay sau** pha `Configure` kênh đầy đủ (trước các ca âm cũ bên dưới,
    //         để số đếm `calls`/`denied` của TAP 6 chỉ gồm sáu invocation của pha này).
    let _ = vifast_user_abi::yield_now();

    // ---- 7b) **KT-314** — `Tcb.Resume` (method 12) **tiêu thụ** `TcbData.entry` (nợ §7.2 của
    //          KT-311). Bốn ca, theo thứ tự **bắt buộc**: ba ca âm chạy khi ta còn `Running`,
    //          đường xanh **cuối cùng** (nó đưa TCB đích lên `Running` ⇒ ta bị preempt về
    //          `Ready`, đúng bất biến ≤1 `Running`/hart của core).
    //
    //          (i) đích `Inactive` **chưa cấu hình `entry`** ⇒ `InvalidArgument` (5).
    let neg_noentry = call(SLOT_TCB_T2, info(METHOD_RESUME, 0, false, 0), [0, 0, 0, 0]);
    if neg_noentry[0] != 5 {
        put(b"# ipc: resume neg noentry err=");
        put_u64(neg_noentry[0]);
        put(b"\n");
        exit(EXIT_RESUME_NOENTRY);
    }
    //          (ii) **chính ta** đang `Running` ⇒ `IllegalOperation` (**17**). Xoá `entry` của ta về
    //          `0` **trước** để chứng minh **kiểm trạng thái đứng trước** kiểm `entry`: nếu thứ
    //          tự sai, kết quả là 5 chứ không phải 7.
    let clear = call(
        SLOT_TCB,
        info(METHOD_CONFIGURE, 0, false, 3),
        [SLOT_FRAME, IPC_BUF_VA, MASK_ENTRY, 0],
    );
    if clear[0] != 0 {
        exit(EXIT_RESUME_RUNNING);
    }
    let neg_running = call(SLOT_TCB, info(METHOD_RESUME, 0, false, 0), [0, 0, 0, 0]);
    if neg_running[0] != 17 {
        put(b"# ipc: resume neg running err=");
        put_u64(neg_running[0]);
        put(b"\n");
        exit(EXIT_RESUME_RUNNING);
    }
    //          (iii) server **đã `Exit`** (`Inactive` + `exit_code`) ⇒ **17** — không tự chạy lại.
    let neg_exited = call(SLOT_TCB_S, info(METHOD_RESUME, 0, false, 0), [0, 0, 0, 0]);
    if neg_exited[0] != 17 {
        put(b"# ipc: resume neg exited err=");
        put_u64(neg_exited[0]);
        put(b"\n");
        exit(EXIT_RESUME_EXITED);
    }
    //          (iv) đường xanh: `Configure(entry? = resume_entry)` rồi `Resume` ⇒ `(0, entry)`.
    //          Kernel nạp `sepc` = `entry` ở lượt chạy kế tiếp của TCB đích.
    let entry_va = resume_entry as usize as u64;
    set_configure_params(0, 0, entry_va, 0);
    let cfg_t = call(
        SLOT_TCB_T,
        info(METHOD_CONFIGURE, 0, false, 3),
        [SLOT_FRAME, IPC_BUF_VA, MASK_ENTRY, 0],
    );
    if cfg_t[0] != 0 {
        put(b"# ipc: resume cfg err=");
        put_u64(cfg_t[0]);
        put(b"\n");
        exit(EXIT_RESUME_FULL);
    }
    let resumed = call(SLOT_TCB_T, info(METHOD_RESUME, 0, false, 0), [0, 0, 0, 0]);
    if resumed[0] != 0 || resumed[1] != entry_va {
        put(b"# ipc: resume full err=");
        put_u64(resumed[0]);
        put(b" val=0x");
        put_hex(resumed[1]);
        put(b"\n");
        exit(EXIT_RESUME_FULL);
    }
    put(b"# ipc: resume ok=1 entry=0x");
    put_hex(entry_va);
    put(b"\n");
    put(b"# ipc: resume neg noentry=");
    put_u64(neg_noentry[0]);
    put(b" running=");
    put_u64(neg_running[0]);
    put(b" exited=");
    put_u64(neg_exited[0]);
    put(b" ok=1\n");

    // ---- 8) Ca âm cũ (giữ nguyên từ KT-280/KT-282): `Call` trên ô **rỗng** ⇒
    //         `InvalidCapability` (1), không chặn.
    let empty = call(SLOT_EMPTY, info(LABEL_REQ, 0, false, 1), [ARG, 0, 0, 0]);
    if empty[0] != 1 {
        exit(EXIT_NEG_EMPTY);
    }
    //         `Call` trên cap **Reply** (không phải Endpoint) ⇒ invocation không có method ⇒
    //         `NotSupported` (18), không chặn, không gửi message nào.
    let wrong = call(SLOT_REPLY, info(LABEL_REQ, 0, false, 1), [ARG, 0, 0, 0]);
    if wrong[0] != 18 {
        exit(EXIT_NEG_REPLY);
    }
    //         `length = 65` vượt `max_words = 64` ⇒ `InvalidArgument` (5) trước khi chạm buffer
    //         — **không tràn** vùng 60 word.
    let over = call(SLOT_EP, info(LABEL_REQ, 0, false, 65), [ARG, 0, 0, 0]);
    if over[0] != 5 {
        exit(EXIT_MR_OVER);
    }
    put(b"# ipc: neg empty=");
    put_u64(empty[0]);
    put(b" reply=");
    put_u64(wrong[0]);
    put(b" ok=1\n");
    put(b"# ipc: neg mr over=");
    put_u64(over[0]);
    put(b" ok=1\n");

    exit(0)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    exit(99)
}
