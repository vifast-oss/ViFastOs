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
//! | 9 | ô **rỗng** — ca âm `Call` trên ô rỗng |
//!
//! Không `unsafe`, không `asm!` (R5): mọi `ecall` qua shim `vifast_user_abi`.
#![no_std]
#![no_main]

extern crate vifast_abi;
extern crate vifast_user_abi;

use vifast_user_abi::{
    call, exit, info, info_label, info_length, init_ipc_buffer, mrs, put, set_extra_caps, set_mrs,
    set_recv_window,
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
/// **KT-290** — VA cố định của trang IPC buffer (loader map `R|W|U|NX` cho mọi ELF U-mode).
const IPC_BUF_VA: u64 = 0x4002_0000;
/// **KT-290** — VA **ngoài** frame IPC buffer (trang kế, chưa map) — ca âm fail-closed.
const IPC_BUF_VA_OUTSIDE: u64 = 0x4002_1000;

/// **KT-309** — slot cap `Frame` **có** `RIGHT_GRANT` (nguồn extra cap đi qua IPC buffer).
const SLOT_FRAME_GRANT: u64 = 5;
/// **KT-309** — slot cap `Frame` **thiếu** `RIGHT_GRANT` — ca âm (i).
const SLOT_FRAME_NO_GRANT: u64 = 6;
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

    // ---- 3) Ca âm (a): `Call` trên ô **rỗng** ⇒ `InvalidCapability` (1), không chặn.
    let empty = call(SLOT_EMPTY, info(LABEL_REQ, 0, false, 1), [ARG, 0, 0, 0]);
    if empty[0] != 1 {
        exit(EXIT_NEG_EMPTY);
    }
    // ---- 4) Ca âm (b): `Call` trên cap **Reply** (không phải Endpoint) ⇒ invocation không có
    // method ⇒ `NotSupported` (18), không chặn, không gửi message nào.
    let wrong = call(SLOT_REPLY, info(LABEL_REQ, 0, false, 1), [ARG, 0, 0, 0]);
    if wrong[0] != 18 {
        exit(EXIT_NEG_REPLY);
    }
    // ---- 5) Ca âm (c) **KT-282**: `length = 65` vượt `max_words = 64` ⇒ `InvalidArgument` (5)
    // trước khi chạm buffer — **không tràn** vùng 60 word.
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
