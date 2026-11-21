# KT-282 — riscv64 **MR4..MR63 qua IPC buffer**

Trạng thái: **DONE** (nhánh `task/kt282-mrbuf`, base `main` @ `20208255`), 2026-11-21.
Sub-agent `mrbuf`. Quyết định: **D2-190** (hợp đồng K3), **D2-205**, **D2-210**, **D2-211**,
**D2-198** (uỷ quyền commit trên nhánh task), **D2-05** (test đỏ trước), **D2-109/D2-111** (tách ảnh
khi thêm đường mới), **D2-83** (core không đọc/ghi bộ nhớ user).

**Không** tuyên bố K3 đạt lại: K3 đã đóng theo D2-205; KT-282 chỉ **trả một nợ K3.5 đã công bố**
(MR4..MR63) và **không** mở lại bất kỳ mục nào của hợp đồng K3.

## 1. Khảo sát trước (bắt buộc) — `vifast-core` đã có gì?

Đọc: `v2/kernel/vifast-core/src/{ipc,core_ipc,core_tcb,syscall,tcbdata,machine,invoke}.rs`,
`v2/kernel/vifast-plat-riscv64/src/{ipc_buffer,user,user_run,core_bridge,mem_bridge}.rs`,
`v2/kernel/vifast-plat-user/lib.rs`, `v2/kernel/vifast-kbin-riscv64/src/ktest_ipc.rs`,
`v2/kernel/vifast-kbin-riscv64/init/ipc_{client,server}_user.rs`, `spec/abi.toml`,
`v2/agent/JOURNAL/kt280-ipc.md`, `v2/test/boot/{ktest.sh,expected.txt,selftest.sh}`.

| Câu hỏi | Trả lời | Bằng chứng |
|---|---|---|
| `vifast-core` có `Tcb.Configure` bind **frame** chưa? | **CHƯA** — `tcb_configure(tcb, cspace)` **chỉ** bind CSpace; `spec/abi.toml` `[method_signature."Tcb.Configure"]` có `ipc_buffer?` nhưng **không** có mã nào đọc tham số đó | `core_tcb.rs:133` (bản trước KT-282), `spec/abi.toml:248` |
| `MessageInfo` có trường `length` > 4 không? | **CÓ** — `length: u8` 7 bit, `MessageInfo::new` nhận `0..=64` (`max_words = 64`); `Message::mr: [u64; 64]`; `IpcArea.buffer: [u64; 60]` | `ipc.rs:19..25`, `ipc.rs:76`, `ipc.rs:318`, `ipc.rs:459` |
| Đường **syscall** có chuyển được > 4 MR chưa? | **CHƯA** — `build_message` từ chối `len > mrs.len()` (`mrs: &[u64]` = 4 thanh ghi) ⇒ `InvalidArgument` (5). Nợ K3.5 ghi rõ ở đầu `syscall.rs` | `syscall.rs:43` (bản trước), `syscall.rs:280` |
| Trang **IPC buffer** đã có chưa? | **CÓ một nửa** — loader map **một** trang 4 KiB `R|W|U|NX` tại `0x4002_0000` cho **mọi** ELF U-mode; chỉ **10 word header** (magic `KT67b`, info, receive window 4, send caps 4) được dùng (KT-67b). Vùng MR4..MR63 **bỏ trống** | `plat-riscv64/src/user.rs:570..590`, `ipc_buffer.rs:23..62` |
| Đường **resume** có nạp MR4..MR63 chưa? | **CHƯA** — `apply_ipc_return` chỉ nạp `a0..a6` (4 MR thanh ghi) | `user_run.rs:32..46` (bản trước) |
| `spec/abi.toml` chốt **offset** vùng MR chưa? | **CHƯA** — `[calling.*].ipc_buffer` chỉ ghi *"Frame bind qua Tcb.Configure; header chứa receive window (CPTR, ≤4 cap)"*. `[message]` chốt `words_in_ipc_buffer = 60`, `ipc_buffer_bytes = 4096`, `ipc_buffer_align = 16` | `spec/abi.toml:35..37, 58` |

⇒ **Kết luận khảo sát**: cả **ba** mảnh đều thiếu — (a) `Tcb.Configure` chưa bind frame, (b) đường
gửi chưa đọc MR4..MR63 từ buffer, (c) đường nhận chưa ghi MR4..MR63 vào buffer. Phải thêm **cả
ba**; **không** hạ phạm vi xuống "MR4..MR7" vì `length = 64` là biên của `max_words` và là ca đột
biến bắt buộc (M2).

## 2. Thiết kế

### 2.1 Layout IPC buffer (`[TK]` KT-282 — spec không chốt offset vùng MR)

```text
byte   0.. 80 : header 10 word (magic, MessageInfo bản ghi, receive window 4 CPTR, send caps 4 CPTR) — KT-67b
byte  80..560 : MR4..MR63 (60 word = 480 B) — KT-282
byte 560..4096: dự phòng (zero)
```

`80 = IPC_HEADER_WORDS * 8` và **đã căn 16** (`[message].ipc_buffer_align = 16`) ⇒ không cần đệm.
Hằng **một nguồn** ở `vifast_core::ipc` (`IPC_BUFFER_MR_OFFSET`/`IPC_BUFFER_MR_WORDS`/
`IPC_BUFFER_MR_END`/`mr_buffer_offset`), shim U-mode (`vifast-plat-user`) dùng **cùng** số
(`MR_OFFSET = HEADER_WORDS * 8`). Không đổi `spec/` (D2-04/D2-22: ABI đóng băng; đây là **model**
`[TK]`, không phải ABI).

### 2.2 Core (`vifast-core`)

| Thay đổi | Tệp |
|---|---|
| `IpcBufferBinding { frame: ObjId, va: u64 }` + 4 hằng + `mr_buffer_offset(i)` | `ipc.rs` |
| `TcbData.ipc_buffer: Option<IpcBufferBinding>` | `tcbdata.rs` |
| `tcb_configure_with_ipc_buffer(tcb, cspace, ipc_buffer: Option<(SlotRef, u64)>)` — `Tcb.Configure` **đầy đủ**; `tcb_configure` cũ **giữ nguyên** (uỷ quyền `None`, 20+ điểm gọi không đổi); `tcb_ipc_buffer_binding(tcb)` | `core_tcb.rs` |
| `SyscallEnv::ipc_buffer_words(tcb, va) -> Option<[u64; 60]>` (mặc định `None`); `build_message` nhận thêm `buffer`; `build_with_header` chỉ đọc buffer khi `length > 4` | `syscall.rs` |
| Tái xuất hằng/kiểu mới | `lib.rs` |

**Kiểm biên** (không nới, không tràn):
- `va % 4096 != 0` hoặc `va + 4096` tràn `u64` ⇒ `InvalidArgument` (5) — **trước** khi ghi gì
  (nguyên tử: CSpace/`exit_code` không đổi).
- `frame_cap` không phải `ObjType::Frame` sống ⇒ `WrongObjectType` (2) / `InvalidCapability` (1).
- `length > 64` ⇒ `InvalidArgument` (5) từ `MessageInfo::unpack` (`max_words = 64`) ⇒ **không**
  chạm 60 word của buffer. `length = 64` là **hợp lệ** (MR0..MR63).
- `length > 4` mà TCB **chưa bind** buffer ⇒ `InvalidArgument` (5) — fail-closed, không đoán.

### 2.3 Arch (`vifast-plat-riscv64`) — D2-83: core không chạm bộ nhớ user

| Thay đổi | Tệp |
|---|---|
| `read_words(va)` / `write_words(va, words)` — đọc/ghi 60 word tại `va + 80`, kiểm `user_range_mapped` + `SUM` (cùng khuôn `read_header`) | `ipc_buffer.rs` |
| `apply_received(tcb)` — chép `IpcArea.buffer[..len-4]` ra buffer user; `apply_received_now` (đường không chặn), `request_write`/`flush_pending` (đường resume) | `ipc_buffer.rs` |
| `SyscallEnv::ipc_buffer_words` → `read_words(va)` (core truyền `va`, arch **không** gọi lại `with_machine` — sẽ tự khoá lồng) | `user.rs` |
| `enter_loaded` gọi `flush_pending()` **ngay sau** `switch_root` (chỗ **duy nhất** `satp` = root user) | `user.rs` |
| `on_u_exception` nhánh `Return`: `nr ∈ 3..=7` ⇒ `apply_received_now(tcb)` | `user.rs` |
| `apply_ipc_return` đặt cờ `request_write(tcb)` thay vì ghi trực tiếp (`satp` = root kernel ở đó) | `user_run.rs` |

**Vì sao cần cờ `PENDING_IPC_BUFFER`**: `apply_ipc_return` chạy **sau** `enter_loaded`, mà
`enter_loaded` đã `restore_kernel_root()` trước khi trả về ⇒ ghi thẳng VA user tại đó là
`StorePageFault` (đã gặp thật: `PANIC: trap StorePageFault scause=0xf stval=0x40020050`). Cờ được
tiêu ở lần vào U-mode **kế tiếp**, đúng một lần, khi `satp` vừa trỏ root user.

### 2.4 Shim U-mode (`vifast-plat-user`)

`MR_OFFSET = 80`, `MR_WORDS = 60`, `set_mrs(&[u64]) -> usize`, `mrs() -> [u64; 60]`,
`mr(i)`, `set_mr(i, v)`. Chương trình U-mode ghi MR4.. **trước** `call`/`send` và đọc **sau**
`recv`/`call` — không thêm syscall, không đổi `MessageInfo` 35 bit.

### 2.5 Ảnh test `ktest-ipc` (tách ảnh theo D2-109/D2-111 — ảnh thứ tám đã có từ KT-280)

Rig thêm **một** `Frame` 4 KiB: object layer `retype(load_untyped, ObjKind::Frame, 12)` (D2-83) →
`mem_bridge::make_key` → `Machine::add_foreign_object(ObjType::Frame, key)` → cap gốc ở ô
`RIG_IPC_BUF = 9` → `tcb_configure_with_ipc_buffer(..., Some((cap, USER_IPC_BUFFER_VA)))` cho **cả
hai** TCB. Scheduler **6 lượt** tất định (KT-280 là 4):

| Lượt | Tác vụ | Việc | `Stop` |
|---|---|---|---|
| 1 | client | `Call` **8 MR** (MR4..MR7 buffer) | `Blocked` |
| 2 | server | `Recv` (message đã trong hàng) → `Yield` | `Yield` |
| 3 | server | `Send` reply **8 MR** → `Recv` vòng 2 | `Blocked` |
| 4 | client | resume: kiểm reply + in `# ipc: mr count=8 …` → `Call` **64 MR** | `Blocked` |
| 5 | server | resume: kiểm 64 MR → `Send` reply **64 MR** → `Exit` | `Exit` |
| 6 | client | resume: kiểm 64 MR + in `# ipc: mr count=64 …` + 3 ca âm → `Exit` | `Exit` |

## 3. Kết quả

| Việc | Kết quả |
|---|---|
| `vifast-core`: `Tcb.Configure` bind frame + `build_message` > 4 MR + kiểm biên | **DONE** |
| `vifast-plat-riscv64`: đọc/ghi vùng MR4..MR63 + đường resume/return | **DONE** |
| `vifast-plat-user`: shim `set_mrs`/`mrs`/`mr`/`set_mr` | **DONE** |
| Ảnh `ktest-ipc`: rig Frame 4 KiB + 6 lượt + TAP 3 | **DONE** — `.data+.bss` = **211.328 B** / 262.144 (headroom 50.816) |
| U-mode client/server: 8 MR + 64 MR + ca âm `length = 65` | **DONE** |
| Runner: `expected.txt` (3 3), `manifest_ipc`, 19 evidence, mẫu PANIC | **DONE** |
| Runner: ca âm thường trực cho dòng `# ipc: mr` (`selftest.sh` ca (l)) | **DONE** — 20/20 ca |
| `./v2/bin/v2 check` | **XANH** (exit 0) |
| Runner riscv64 `ktest.sh 1 matrix` | **XANH — 24/24 boot sạch** |
| Ba đột biến | **ĐỎ đúng cả ba** (§6) |

**Ghi chú bắt buộc — `size_of::<Machine>()` 189.384 → 190.920 B**: thêm **một**
`Option<IpcBufferBinding>` (24 B) mỗi TCB ⇒ `+24 × MAX_TCBS = +1.536 B`. Pin **196.608 B không
đổi** (headroom còn 5.688 B); `tls_tests::pin_machine_tang_dung_8_byte_moi_tcb` cập nhật **phép
tính** (không nới ngưỡng) và 4 dòng evidence `bytes=/static=` trong `ktest.sh` đổi theo **số đo
thật** — cùng cách KT-74 đã làm khi `188.872 → 189.384`. `.data+.bss` ảnh chính **246.936 B** ≤
262.144 (headroom 15.208).

## 4. Log QEMU nguyên văn (smp=1, ảnh `ktest-ipc`, commit `de741173`)

```text
TAP version 13
1..3
# ipc rig: stvec=0x802000a0 untyped=0 core_untyped=1 cspace_c=11 cspace_s=12 tcb_c=3 tcb_s=4 ep=5 reply=6 ipc_buf=10
# ipc call: blocked=1 state=BlockedOnSend call=1 ep_len=1 ep_mr=0x1234 label=0x280 stop=Blocked
# ipc server: recv=1 label=0x280 mr=0x1234 client=BlockedOnReply caller=3 server=Running stop=Yield
# ipc: server ran nr=1
# ipc reply: client=Ready reply_caller=0 server=BlockedOnRecv stop=Blocked
# ipc: umode call ok reply=23130
# ipc: mr count=8 mr7=0x28210007
# ipc mr: round=1 srv_len=8 srv_mr4=0x28200004 srv_mr7=0x28200007 cli_len=8 cli_mr4=0x28210004 cli_mr7=0x28210007
# ipc client: reply_label=0x281 reply_mr0=0x5a5a reply_mr1=1 const:label_reply=0x281 const:reply_value=0x5a5a
# ipc: server ran nr=2
# sys: nr=14 err=0 value=0 tcb=4 state=Inactive src=core
# u-mode exit: code=0 faultkind=Exit{0} tcb=4 state=Inactive
# ipc mr: round=2 srv_len=64 srv_mr4=0x28200004 srv_mr63=0x2820003f
# ipc: mr count=64 mr63=0x2821003f
# ipc: neg empty=1 reply=18 ok=1
# ipc: neg mr over=5 ok=1
# sys: nr=14 err=0 value=0 tcb=3 state=Inactive src=core
# u-mode exit: code=0 faultkind=Exit{0} tcb=3 state=Inactive
# ipc mr: round=2 cli_len=64 cli_mr4=0x28210004 cli_mr63=0x2821003f
# ipc buf: bound=1 va=0x40020000 mr_off=80 mr_words=60
ok 1 - ipc: umode call
ok 2 - ipc: server replied
ok 3 - ipc: mr4..mr7 via buffer
# ipc final: client_exit=0 server_exit=0 reply_clean=1 check_ipc=1 check_caps=1 capture_client=1 capture_server=1 client=Inactive server=Inactive
PANIC: KT-282 ipc u-mode + mr buffer xong (3 TAP, 0 fail)
```

Đọc dòng theo dòng (mỗi dòng là một **phép đo**, không phải lời khai):

1. `# ipc rig: … ipc_buf=10` — rig dựng **thêm** object `Frame` 4 KiB (id 10) làm IPC buffer.
2. `# ipc call: … ep_len=1 ep_mr=0x1234` — client `Call` **8 MR** chặn ở Send; message nằm trong
   hàng Endpoint. `ep_mr` = MR0 (thanh ghi) — MR4..MR7 nằm trong buffer, xem dòng 7.
3. `# ipc server: recv=1 … client=BlockedOnReply` — server `Recv` lấy message, caller chuyển
   `BlockedOnSend{call}` → `BlockedOnReply`.
4. `# ipc: server ran nr=1` — **chính server U-mode** in (qua `DebugPutStr`).
5. `# ipc reply: client=Ready reply_caller=0 server=BlockedOnRecv stop=Blocked` — server `Send`
   reply 8 MR ⇒ client `Ready`, `Reply.caller` sạch; server sang `Recv` vòng 2 và chặn.
6. `# ipc: umode call ok reply=23130` — **chính client U-mode** in sau khi **được resume**.
7. `# ipc: mr count=8 mr7=0x28210007` — **chính client** đọc **IPC buffer của nó** (offset 80) và
   thấy MR7 = `0x28210007` = `REPLY_MR7` ⇒ kernel đã ghi MR4..MR7 vào buffer khi resume.
8. `# ipc mr: round=1 srv_len=8 srv_mr4=0x28200004 srv_mr7=0x28200007 cli_len=8 cli_mr4=0x28210004
   cli_mr7=0x28210007` — kernel đọc **độc lập** `tcb_ipc_area` của **cả hai** bên: server nhận
   đúng `REQ_MR4/REQ_MR7`, client nhận đúng `REPLY_MR4/REPLY_MR7`. MR4..MR7 **chỉ** có thể đến từ
   `SyscallEnv::ipc_buffer_words` (arch đọc trang user) ⇒ dòng này chứng minh đường **buffer**,
   không phải đường thanh ghi.
9. `# ipc: server ran nr=2` — server phục vụ xong vòng **64 MR**.
10. `# ipc mr: round=2 srv_len=64 srv_mr4=0x28200004 srv_mr63=0x2820003f` — biên trên: `length =
    64` (MR0..MR63) đi qua buffer, MR63 = word 59 = `REQ_MR63`.
11. `# ipc: mr count=64 mr63=0x2821003f` — client đọc buffer của nó: MR63 = `REPLY_MR63`.
12. `# ipc: neg empty=1 reply=18 ok=1` — hai ca âm **giữ nguyên** từ KT-280.
13. `# ipc: neg mr over=5 ok=1` — **ca âm mới**: `length = 65` ⇒ `InvalidArgument` (5), **không**
    tràn vùng 60 word.
14. `# ipc mr: round=2 cli_len=64 cli_mr4=0x28210004 cli_mr63=0x2821003f` — kernel đọc độc lập
    phía client vòng 2.
15. `# ipc buf: bound=1 va=0x40020000 mr_off=80 mr_words=60` — `Tcb.Configure` **đã** bind frame
    (VA `0x4002_0000`), offset vùng MR = 80, 60 word.
16. `# ipc final: … check_ipc=1 check_caps=1 …` — bất biến core xanh, hai TCB `Inactive`.

## 5. TAP + matrix

```text
$ bash v2/test/boot/ktest.sh 1 matrix
# expected[ktest-ipc]: plan=3 ok=3 ten-TAP=3
REV-GATE: ACCEPT inputs=016242057ed6041e current_inputs=016242057ed6041e head_rev=de74117332f2 count=311
ok - ktest       smp=1 boot 1 (70 TAP, 5s)  log …/ktest-smp1-1.log
ok - ktest-guard smp=1 boot 1 (3 TAP, 1s)   log …/ktest-guard-smp1-1.log
ok - ktest-user  smp=1 boot 1 (37 TAP, 1s)  log …/ktest-user-smp1-1.log
ok - ktest-proc  smp=1 boot 1 (10 TAP, 1s)  log …/ktest-proc-smp1-1.log
ok - ktest-vblk  smp=1 boot 1 (6 TAP, 0s)   log …/ktest-vblk-smp1-1.log
ok - ktest-con   smp=1 boot 1 (5 TAP, 1s)   log …/ktest-con-smp1-1.log
ok - ktest-exec  smp=1 boot 1 (6 TAP, 1s)   log …/ktest-exec-smp1-1.log
ok - ktest-ipc   smp=1 boot 1 (3 TAP, 1s)   log …/ktest-ipc-smp1-1.log
… (smp=2, smp=4 cùng 8 ảnh)
# ktest-ipc   smp=1  1/1 boot sạch (FAIL=0 TIMEOUT=0)
# ktest-ipc   smp=2  1/1 boot sạch (FAIL=0 TIMEOUT=0)
# ktest-ipc   smp=4  1/1 boot sạch (FAIL=0 TIMEOUT=0)
# KTEST: 24/24 boot sạch (smp=matrix, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec ktest-ipc)
# ktest.sh: PASS
```

**Ca âm thường trực** (`bash v2/test/boot/selftest.sh`, 20/20):

```text
ok 18 - log ktest-ipc: xoá dòng '# ipc: umode call ok reply=' (evidence bắt buộc của KT-280)
#   runner đỏ vì [ev6]: not ok - check-log ktest-ipc smp=1: thiếu ev6, log …/log-ipc-no-call.log
ok 19 - log ktest-ipc: xoá dòng '# ipc: mr count=8 mr7=' (evidence bắt buộc của KT-282)
#   runner đỏ vì [ev7]: not ok - check-log ktest-ipc smp=1: thiếu ev7, log …/log-ipc-no-mr.log
ok 20 - expected.txt nguyên vẹn sau mutation: sha256 67f9688d138fcac3… (trước = sau) + cmp
# selftest: 20/20 ca đúng như kỳ vọng
# selftest.sh: PASS
```

`--check-log` (19 evidence bắt buộc):

```text
$ bash v2/test/boot/ktest.sh --check-log ktest-ipc /tmp/kt282-ipc.log 1
ok - check-log ktest-ipc smp=1 (plan=3 ok=3, 19 evidence) log /tmp/kt282-ipc.log
# check-log: PASS
```

**Ca âm thường trực** trong `v2/test/boot/selftest.sh`:
- ca (k) **giữ nguyên** (KT-280): xoá `# ipc: umode call ok reply=…` ⇒ đỏ token `ev6`;
- ca (l) **mới** (KT-282): xoá `# ipc: mr count=8 mr7=…` ⇒ đỏ token `ev<ordinal>` (ordinal tra từ
  `--print-evidence`, không hardcode).

## 6. Đột biến (đỏ có kiểm soát) — cả ba ĐỎ đúng token

Script: `/tmp/kt282-mutations.sh` (sửa **một** chỗ → build ảnh `ktest-ipc` → boot thật qua
`qemu_guard.sh` → `ktest.sh --check-log` phải đỏ → `git checkout --` trả tệp). Cây sạch sau cả ba.

| # | Đột biến | Kết quả log | Runner |
|---|---|---|---|
| M1 | **không bind buffer** (`tcb_configure` thay `tcb_configure_with_ipc_buffer`) | `# ipc: client call err=5`; `# u-mode exit: code=80`; `# ipc buf: bound=0 va=0x0` | **ĐỎ**: thiếu TAP #1/#2/#3 + evidence#2 |
| M2 | `length = 64` bị từ chối (`>= MAX_WORDS` thay `>`) | vòng 1 xanh (`# ipc: mr count=8 mr7=0x28210007`), rồi `# u-mode exit: code=88` (client `Call` 64 MR lỗi) | **ĐỎ**: thiếu TAP #2/#3 + evidence#10/#11 |
| M3 | buffer trỏ **ra ngoài frame** (`va = 0x4002_0000 + 4096`, trang chưa map) | `# ipc: client call err=5`; `# ipc buf: bound=1 va=0x40021000` | **ĐỎ**: thiếu TAP #1/#2/#3 + evidence#2 |

M1 chính là **hành vi trước KT-282** (`length > 4` ⇒ `InvalidArgument`) ⇒ đây là **bằng chứng đỏ
trước** (D2-05): TAP 3 không thể xanh nếu chưa có đường buffer. M2 chứng minh biên trên `length =
64` là **hợp lệ** (không phải "vượt 63 là lỗi"): chỉ `length > 64` mới lỗi. M3 chứng minh
`read_words` **kiểm dải đã map** trước khi đọc (fail-closed), không đọc bừa ngoài frame.

## 7. Còn gì chưa xong / rủi ro

1. **Không** làm (đúng phạm vi task): cap transfer qua buffer, nhiều buffer/TCB, scatter-gather,
   `Tcb.Configure` map frame **thật** vào VSpace (loader đã map sẵn trang 4 KiB tại VA cố định —
   binding chỉ giữ **danh tính** frame + VA, đúng D2-83: core không sở hữu PTE/PA).
2. **Không** tuyên bố K3 đạt lại — K3 đã đóng theo D2-205.
3. **Nợ**: `Tcb.Configure` chưa có **đường invocation** từ U-mode (`invoke.rs` danh sách trắng
   chưa có `Tcb.Configure`) — rig gọi thẳng API core như mọi ảnh test khác; thêm invocation là
   việc của task khác (chạm `invoke.rs` + `spec`).
4. **Nợ**: `apply_ipc_return` vẫn chỉ nạp `a0..a6`; MR4..MR63 đi qua **cờ** `PENDING_IPC_BUFFER`
   (một TCB một lúc — đúng mô hình một-hart của ảnh test). Nếu sau này có nhiều tác vụ chặn đồng
   thời trên nhiều hart, cờ này phải thành **per-hart** hoặc ghi ngay trong trap handler.
5. **Rủi ro thấp**: `size_of::<Machine>()` tăng 1.536 B (pin 196.608 **không** đổi, còn 5.688 B
   headroom); `.data+.bss` ảnh chính 246.936 / 262.144 (còn 15.208 B).
6. **Chưa chạy T2 `gate --fast`** (task yêu cầu `v2 check` + runner riscv64; T2 là việc của lead
   trên ứng viên tích hợp).

## 8. Xác nhận luật

- **Không** push, **không** merge, **không** chạm `main` (nhánh `task/kt282-mrbuf`).
- **Không** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `v2/bin/v2`, **không** sửa
  `spec/`.
- **Không** nới ngưỡng (pin `196.608` và trần `.data+.bss` `262.144` **không đổi**), **không**
  `#[ignore]`/retry, **không** `cfg(target_arch)` trong `vifast-core`/`vifast-plat-common`.
- Stage theo **đường dẫn cụ thể** (16 tệp), **không** `git add -A`.
- Cây sạch sau commit + sau ba đột biến (`git status --short` rỗng).
