# KT-311 — riscv64 **`Tcb.Configure` kênh đầy đủ** (`cspace?`/`entry?`/`priority?`)

Trạng thái: **đang làm** (nhánh `task/kt311-cfgfull`, base `main` @ `81b8e210`), 2026-11-22.
Sub-agent `cfgfull`. Quyết định: **D2-190** (hợp đồng K3), **D2-205** (K3 đã đóng), **D2-221**
(vòng 20 mở KT-311), **D2-198** (uỷ quyền commit trên nhánh task), **D2-05** (test đỏ trước),
**D2-83** (core không đọc/ghi bộ nhớ user), **D2-104** (arity chính xác), **D2-04/D2-22**
(`spec/` đóng băng), **D2-11** (số method/hằng lấy từ spec, không chép tay).

**Không** tuyên bố K3 đạt lại: K3 đã đóng theo D2-205; KT-311 chỉ **trả một nợ K3.5 đã công bố**
(KT-290 §7.1: *"`cspace?`/`vspace?`/`entry?`/`priority?` chưa có đường invocation"*) và **không**
mở lại bất kỳ mục nào của hợp đồng K3.

**Ghi chú đọc trước**: `v2/agent/DECISIONS.md` **trong worktree** (base `81b8e210`) dừng ở
**D2-220**; **D2-221** chỉ có trên `origin/main` (`f1f5e729`) — đã đọc bản `origin/main`:
D2-221(2) mở KT-311 với đúng câu *"nếu cần đổi ABI thì DỪNG và báo (D2-04)"*. Nhánh task
**không** rebase/merge (luật của phiếu); lead merge. Không sửa tệp DECISIONS (ngoài quyền của
sub-agent).

## 1. Khảo sát trước (bắt buộc)

Đọc: `spec/abi.toml` (`[method_signature."Tcb.Configure"]`, `[object].Tcb`, `[error]`,
`[message]`), `v2/kernel/vifast-core/src/{invoke,core_tcb,tcbdata,ipc,syscall,error,footprint,
tls_tests,configure_tests}.rs`, `v2/kernel/vifast-plat-riscv64/src/{ipc_buffer,user}.rs`,
`v2/kernel/vifast-plat-user/lib.rs`, `v2/kernel/vifast-kbin-riscv64/src/ktest_ipc.rs`,
`v2/kernel/vifast-kbin-riscv64/init/{ipc_client_user,ipc_server_user}.rs`,
`v2/test/boot/{ktest.sh,expected.txt,selftest.sh}`, `v2/agent/JOURNAL/{kt290-cfg,kt309-captransfer}.md`.

| Câu hỏi | Trả lời | Bằng chứng |
|---|---|---|
| Chữ ký `Tcb.Configure` có mấy tham số? | **6** tên, **5** tuỳ chọn: `tcb`, `cspace?`, `vspace?`, `entry?`, `ipc_buffer?`, `priority?`; `returns = []` | `spec/abi.toml:247..249` |
| KT-290 dùng bao nhiêu word cho `ipc_buffer?`? | **2**: `MR0` = CPTR cap `Frame` (CSpace caller), `MR1` = VA căn 4 KiB; `length` **chính xác 2** | `invoke.rs:399..450`, `kt290-cfg.md:42..57` |
| Còn chỗ trong **4 MR** không? | `MR0`/`MR1` đã dùng cho `ipc_buffer?` ⇒ còn **2** word (`MR2`/`MR3`). Bốn tham số còn lại cần **4** word (mỗi CPTR/VA/u8 một word) ⇒ **KHÔNG đủ** trong 4 MR. `MessageInfo` 35 bit cũng không còn trường trống (D2-99: `label`/`caps`/`no_reply`/`extra_caps_unwrapped`/`length` đều có chủ) | `spec/abi.toml:44..48`, `ipc.rs:150..173` |
| IPC buffer còn vùng trống không? | **CÓ** — KT-309 đã dùng `byte 560..608`; vùng `byte 608..4096` **trống hoàn toàn** (zero, cùng trang 4 KiB đã map `R|W|U|NX` cho mọi ELF U-mode) | `ipc.rs:42..49,76..85`, `kt309-captransfer.md:60..73` |
| `TcbData` đã có trường cho 4 tham số đó chưa? | `cspace: Option<ObjId>` **có**; `priority: u8` **có** (kèm `max_controlled_priority: u8`); `entry` **KHÔNG có**; `vspace` **KHÔNG có** (VSpace thuộc arch — D2-83) | `tcbdata.rs:20..60` |
| Ngữ nghĩa `priority` đã có ở đâu? | `Machine::tcb_set_priority(tcb, priority, mcp)` — `mcp < priority` ⇒ `InvalidArgument` (MCS, KT-21/59); `sched_sync` cập nhật bucket | `core_tcb.rs:315..342` |
| `cspace?` gắn được bằng API nào? | `require_type(slot, ObjType::CNode)` rồi gán `TcbData.cspace` — đúng ngữ nghĩa `tcb_configure_with_ipc_buffer(..., Some(cspace), ...)` | `core_tcb.rs:152..172` |
| Ngân sách tĩnh còn bao nhiêu? | `size_of::<Machine>()` = **194.520** / pin **196.608** (headroom **2.088**); `TcbData` = **816** B; `.data+.bss` ảnh `ktest-ipc` = **215.064** / 262.144 | `tls_tests.rs:332..360`, `ktest.sh:589,602,632,797` |

⇒ **Kết luận khảo sát**: (a) **không** đủ chỗ trong 4 MR cho 4 tham số còn lại ⇒ phải dùng
**vùng riêng** của IPC buffer (đúng khuôn `[TK]` KT-282/KT-309, **không** đổi ABI, **không**
chạm `spec/`); (b) `cspace?`/`priority?` đã có trường + API core, `entry?` cần **một** trường
model mới (`u64`/TCB ⇒ `+512 B`, pin không đổi), `vspace?` **không** có trường và VSpace thuộc
arch (D2-83) ⇒ **không cài** trong task này (ghi nợ §7); (c) kênh mới phải **giữ nguyên** đường
`length = 2` của KT-290 (20+ điểm gọi/ảnh cũ không đổi).

## 2. Thiết kế (kênh hẹp — ghi rõ)

### 2.1 Kênh tham số (`[TK]` KT-311 — nối tiếp KT-290, **không** đổi ABI)

`Tcb.Configure` qua invocation (syscall 0 trên cap TCB) nhận **hai** dạng `length`:

| `length` | Nghĩa | MR |
|---|---|---|
| **2** | KT-290 (giữ nguyên): chỉ `ipc_buffer?` | `MR0` CPTR `Frame`, `MR1` VA |
| **3** | **đầy đủ**: `ipc_buffer?` + mask tham số mở rộng | `MR0`/`MR1` như trên, `MR2` = **mask** |
| khác | `InvalidArgument` **5** (D2-104) | — |

`MR2` (mask) — bit nào bật thì tham số đó **có mặt**; giá trị đọc từ **vùng riêng** của IPC
buffer (byte 608..640, cùng trang với header KT-67b + extra caps KT-309):

| Bit | Tham số | Word trong buffer | Kiểm |
|---|---|---|---|
| 0 | `cspace?` | 608..616 CPTR (CSpace **caller**) | cap `CNode` sống ⇒ không: `WrongObjectType` **2** / `InvalidCapability` **1** |
| 1 | `vspace?` | 616..624 (dự phòng) | **chưa cài** ⇒ bật bit ⇒ `NotSupported` **18** |
| 2 | `entry?` | 624..632 VA | căn **4 B** ⇒ không: `Alignment` **6** |
| 3 | `priority?` | 632..640 | phải vừa `u8` **và** ≤ `max_controlled_priority` ⇒ không: `InvalidArgument` **5** |

Bit ngoài `0..3` ⇒ `InvalidArgument` **5**. Mask = 0 ⇒ tương đương `length = 2` (chỉ bind
buffer). **Nguyên tử**: mọi tham số được kiểm **trước** khi ghi bất cứ trường nào (cùng luật
`tcb_configure_with_ipc_buffer`).

**Vì sao mask ở `MR2` mà không phải 4 MR**: `MR0`/`MR1` là của `ipc_buffer?` (KT-290, giữ
nguyên); `MR2` là word **duy nhất** còn dùng được mà không đổi `MessageInfo` 35 bit (D2-99) và
không đổi `spec/` (D2-04). Bốn **giá trị** nằm trong vùng dự phòng của IPC buffer — đúng
phương án D2-221(2) cho phép (*"nếu cần đổi ABI thì DỪNG"* — ở đây **không** cần).

### 2.2 Layout IPC buffer (`[TK]` KT-311 — nối tiếp KT-282/KT-309)

```text
byte   0.. 80 : header 10 word (magic, MessageInfo bản ghi, receive window 4 CPTR, send caps 4 CPTR) — KT-67b
byte  80..560 : MR4..MR63 (60 word = 480 B) — KT-282
byte 560..568 : extra_caps count n (0..=4) — KT-309
byte 568..600 : n CPTR nguồn (4 × 8 B) — KT-309
byte 600..608 : extra_caps dst — KT-309
byte 608..616 : **cspace CPTR** (CSpace caller) — KT-311
byte 616..624 : **vspace CPTR** (dự phòng — chưa cài) — KT-311
byte 624..632 : **entry** (VA, căn 4 B) — KT-311
byte 632..640 : **priority** (u8) — KT-311
byte 640..4096: dự phòng (zero)
```

Hằng **một nguồn** ở `vifast_core::ipc` (`IPC_BUFFER_CONFIGURE_OFFSET` = `IPC_BUFFER_EXTRA_CAPS_END`
= 608, `IPC_BUFFER_CONFIGURE_WORDS` = 4, `IPC_BUFFER_CONFIGURE_END` = 640, bốn hằng mask
`CONFIGURE_MASK_*`); shim U-mode dùng **cùng** số. **Không** đổi `spec/`.

### 2.3 Ngữ nghĩa

1. `length = 2` ⇒ đường KT-290 **nguyên vẹn** (`tcb_configure_ipc_buffer`, giữ CSpace).
2. `length = 3` ⇒ đọc mask `MR2`; mask = 0 ⇒ như (1); mask ≠ 0 ⇒ arch đọc 4 word vùng 608..640
   (D2-83: core không đọc bộ nhớ user) rồi kiểm **tất cả** trước khi ghi.
3. `cspace?` ghi `TcbData.cspace` = object `CNode` của CPTR (CPTR giải trong CSpace **caller**,
   như `ipc_buffer?` của KT-290).
4. `entry?` ghi `TcbData.entry` (model-only `[TK]`; **chưa** có đường `Resume` tiêu thụ — ghi nợ
   §7.2) — căn 4 B (biên lệnh RISC-V; ảnh test link tại `0x4000_0000`).
5. `priority?` gọi `Machine::tcb_set_priority(tcb, prio, mcp)` — **giữ** `mcp`, cập nhật bucket
   scheduler (`sched_sync`), không đổi trạng thái TCB.
6. `Configure` vẫn **xoá `exit_code`** (D2-25) và **không** đổi trạng thái TCB/scheduler.

### 2.4 Đo

- Client U-mode in: `# ipc: configure2 ok=1 entry=0x… prio=…` + `# ipc: configure2 neg align=6
  prio=5 cspace=2 vspace=18 ok=1`.
- Kernel đo **độc lập**: `# ipc: configure2 entry=0x… prio=<n> cspace=<objid> vspace=0 calls=<n>
  denied=<n> ok=1` (đọc lại `TcbData.entry`/`priority`/`cspace` sau lượt U-mode).
- TAP mới: `ok 6 - ipc: tcb configure full`.

### 2.5 Ca âm (trong cùng boot)

| # | Ca | Kỳ vọng |
|---|---|---|
| (i) | `entry = 0x4000_0002` (không căn 4 B) | `Alignment` **6** |
| (ii) | `priority = 256` (ngoài `u8`) | `InvalidArgument` **5** |
| (iii) | `cspace` = cap `Frame` (không phải `CNode`) | `WrongObjectType` **2** |
| (iv) | mask bit 1 (`vspace?` — chưa cài) | `NotSupported` **18** |

Giữ nguyên **toàn bộ** ca âm cũ của KT-280/KT-282/KT-290/KT-309.

## 3. Kết quả

| Hạng mục | Kết quả |
|---|---|
| Core: `TcbConfigure` + `tcb_configure_invocation` + `tcb_entry` + `TcbData.entry` | **DONE** — kiểm **tất cả** trước khi ghi (nguyên tử), `Configure` vẫn xoá `exit_code` (D2-25) |
| Core: `invoke::configure` kênh `length = 3` (mask `MR2`) | **DONE** — mask lạ ⇒ 5; `vspace?` ⇒ 18; arch không đọc được vùng ⇒ 5 (fail-closed) |
| Core: `IpcHeader.configure` (vùng 608..640 đọc **cùng lần** với header — D2-83) | **DONE** — không thêm hook `SyscallEnv` mới (giữ `user.rs` ≤ 1.500 dòng, R1) |
| Arch: `ipc_buffer::read_configure_params` + shim `set_configure_params` | **DONE** — một layout, hai phía; hằng `IPC_BUFFER_CONFIGURE_*`/`CONFIGURE_MASK_*` ở `vifast_core::ipc` |
| Client U-mode: 4 ca âm + đường xanh + chứng minh `cspace?` đổi CSpace + trả về | **DONE** — `# ipc: configure2 ok=1 …`, `# ipc: configure2 neg …` |
| Rig: `cnode_alt` (ô 0 = CSpace gốc, ô 3 = TCB, ô 4 = Frame, ô 8 = Reply) + đo độc lập | **DONE** — `# ipc: configure2 entry=… prio=… cspace=… vspace=0 calls=2 denied=5 ok=1` |
| TAP mới `ok 6 - ipc: tcb configure full` | **DONE** — ảnh `ktest-ipc` **6/6** TAP, 31 evidence |
| Runner: `expected.txt` 5→6, `manifest_ipc` 6, evidence mới, mẫu PANIC mới, `selftest.sh` ca (p) | **DONE** |
| `cargo test -p vifast-core` | **XANH — 67/67** (6 test mới cho KT-311) |
| `./v2/bin/v2 check` (cây **sạch**, HEAD `5689723a`) | **XANH** (exit 0, `rules: PASS`) |
| Runner riscv64 `ktest.sh 1 matrix` (cây **sạch**, HEAD `5689723a`, `KT54_LOGDIR` riêng) | **XANH — 27/27 boot sạch** |
| `selftest.sh` | **XANH — 24/24 ca** (ca (p) mới: `ev28`) |
| Ba đột biến | **ĐỎ đúng cả ba** (§6) |
| `size_of::<Machine>()` | 194.520 → **195.032 B** (+512 = `TcbData.entry: u64` × 64 TCB); pin **196.608 không đổi** (headroom 1.576) |
| `.data+.bss` ảnh `ktest-ipc` | 215.064 → **215.576 B** / 262.144 (headroom 46.568) |

**Ghi chú bắt buộc — `size_of::<Machine>()` 194.520 → 195.032 B** (+512): `TcbData` thêm **một**
`u64` (`entry`, căn 8 ⇒ 8 B × `MAX_TCBS` 64 = **512**). Pin **196.608 không đổi**;
`tls_tests::pin_machine_tang_dung_8_byte_moi_tcb` cập nhật **phép tính** (không nới ngưỡng) và
**bốn** dòng evidence `bytes=`/`sum=`/`static=` trong `ktest.sh` đổi theo **số đo thật**
(`194520` → `195032`) — cùng cách KT-282/KT-309 đã làm.

**Ghi chú hạ tầng (T-INFRA)**: lần chạy matrix **đầu** đỏ 3 boot ảnh `ktest` với `thiếu ev76`
(`bytes=184280`) — log bị **ghi đè** bởi một lượt matrix **khác** chạy song song trong repo chính
`/Users/mps/dvchd/ViFastOs` (HEAD `080c7da3` = KT-312, `size_of::<Machine>()` = 184.280) vì
`LOGDIR` mặc định `$TMPDIR/k3-ktest` **dùng chung**. Chạy lại với `KT54_LOGDIR=/tmp/kt311-logs`
**riêng** ⇒ 27/27 sạch. Không phải lỗi mã; đã ghi để vòng sau biết (đề xuất: runner nên có
`LOGDIR` riêng theo PID/phiên — ngoài phạm vi KT-311).

## 4. Log QEMU nguyên văn (smp=1, ảnh `ktest-ipc`, HEAD `5689723a`)

```text
TAP version 13
1..6
# ipc rig: stvec=0x802000a0 untyped=0 core_untyped=1 cspace_c=11 cspace_s=12 cspace_alt=13 tcb_c=3 tcb_s=4 ep=5 reply=6 ipc_buf=10
# ipc: configure ok va=0x40020000
# ipc: configure err=0
# ipc: configure neg cap=18 method=18 outside=5 ok=1
# ipc configure: method=0 delta=3 denied=2 bound=1 va=0x40020000
# ipc call: blocked=1 state=BlockedOnSend call=1 ep_len=1 ep_mr=0x1234 label=0x280 stop=Blocked
# ipc server: recv=1 label=0x280 mr=0x1234 client=BlockedOnReply caller=3 server=Running stop=Yield
# ipc: server ran nr=1
# ipc reply: client=Ready reply_caller=0 server=BlockedOnRecv stop=Blocked
# ipc: umode call ok reply=23130
# ipc: mr count=8 mr7=0x28210007
# ipc mr: round=1 srv_len=8 srv_mr4=0x28200004 srv_mr7=0x28200007 cli_len=8 cli_mr4=0x28210004 cli_mr7=0x28210007
# ipc client: reply_label=0x281 reply_mr0=0x5a5a reply_mr1=1 const:label_reply=0x281 const:reply_value=0x5a5a
# ipc: server ran nr=2
# ipc mr: round=2 srv_len=64 srv_mr4=0x28200004 srv_mr63=0x2820003f
# ipc: mr count=64 mr63=0x2821003f
# ipc mr: round=2 cli_len=64 cli_mr4=0x28210004 cli_mr63=0x2821003f
# ipc: cap busy=7 ok=1
# ipc: cap used slot=5 copy=6
# sys: nr=14 err=0 value=0 tcb=4 state=Inactive src=core
# u-mode exit: code=0 faultkind=Exit{0} tcb=4 state=Inactive
# ipc caps: sent=1 received=1 slot=5 rights=0xb obj=10 ty=Frame alive=1 copy_slot=6 copy_obj=10 copy_ty=Frame same=1
# ipc: cap sent=1 slot=5
# ipc: cap neg grant=3 over=5 ok=1
# ipc: configure2 ok=1 entry=0x40000112 prio=4
# ipc: configure2 neg align=6 prio=5 cspace=2 vspace=18 ok=1
# ipc: configure2 entry=0x40000112 prio=4 cspace=11 vspace=0 calls=2 denied=5 ok=1
# ipc: neg empty=1 reply=18 ok=1
# ipc: neg mr over=5 ok=1
# sys: nr=14 err=0 value=0 tcb=3 state=Inactive src=core
# u-mode exit: code=0 faultkind=Exit{0} tcb=3 state=Inactive
# ipc buf: bound=1 va=0x40020000 mr_off=80 mr_words=60
ok 1 - ipc: umode call
ok 2 - ipc: server replied
ok 3 - ipc: mr4..mr7 via buffer
ok 4 - ipc: tcb configure via invocation
ok 5 - ipc: cap transfer
ok 6 - ipc: tcb configure full
PANIC: KT-311 ipc u-mode + mr buffer + tcb configure + cap transfer + configure full xong (6 TAP, 0 fail)
```

**Đọc dòng `# ipc: configure2 entry=0x40000112 prio=4 cspace=11 vspace=0 calls=2 denied=5 ok=1`**:
`entry` = **`e_entry` thật** của `ipc_client.elf` (`0x4000_0112`, đọc từ ELF — client dùng
`_start as usize`, rig đối chiếu `LoadedInit.entry`); `prio` = 4 (client đặt, ≤ `mcp` = 10);
`cspace` = **11** = `cnode_c` (**đã trả về** sau khi đổi sang `cnode_alt` = 13); `vspace` = 0
(**chưa cài** — in 0, không giả); `calls` = 2 (đường xanh + trả `cspace`); `denied` = 5 (bốn ca
âm + một `Call` trên cap `Reply` để chứng minh `cspace` đã đổi). `smp=2/4` in **cùng** dòng.

## 5. TAP + matrix

- Ảnh `ktest-ipc`: **6/6 TAP**, **31** evidence (thêm 3: `# ipc: configure2 ok=…`,
  `# ipc: configure2 neg …`, `# ipc: configure2 entry=…`), PANIC mẫu mới.
- `expected.txt`: `ktest-ipc 6 6` (5 → 6); `manifest_ipc` thêm `ipc: tcb configure full`.
- `ktest.sh 1 matrix` (HEAD `5689723a`, `KT54_LOGDIR=/tmp/kt311-logs`): **27/27 boot sạch**
  (9 ảnh × smp 1/2/4), `ktest.sh: PASS`.
- `selftest.sh`: **24/24 ca**, gồm ca (p) mới — xoá dòng `# ipc: configure2 entry=0x` ⇒ runner
  **đỏ** đúng token `ev28`.

## 6. Đột biến (ba ca, mỗi ca sửa **một** chỗ rồi build + boot thật + `--check-log`)

| # | Đột biến | Kết quả | Bằng chứng |
|---|---|---|---|
| M1 | Bỏ kiểm **căn lề `entry`** (`entry % TCB_ENTRY_ALIGN`) | **ĐỎ** | `# ipc: configure2 neg align err=0` (kỳ vọng 6) ⇒ `not ok 6 … entry=0x40000001 … client_exit=100` |
| M2 | Bỏ kiểm **dải `u8` của `priority`** (`u8::try_from` → `as u8`, cắt ngầm 256 → 0) | **ĐỎ** | `# ipc: configure2 neg prio err=0` (kỳ vọng 5) ⇒ `not ok 6 … prio=0 … client_exit=101` |
| M3 | Bỏ kiểm **loại `cspace`** (`require_type(CNode)` → `cap_obj_at`) | **ĐỎ** | `# ipc: configure2 neg cspace err=0` (kỳ vọng 2) ⇒ `not ok 6 … cspace=10 … client_exit=102` |

Mỗi ca: `git status` tệp đã chạm **nguyên vẹn** sau khôi phục (`cmp` với bản sao). Ba mã thoát
`100/101/102` là ca âm của client — chứng minh **đúng** ca âm bị phá, không phải đỏ vô tội vạ.

## 7. Còn gì chưa xong / rủi ro

1. **`vspace?` CHƯA cài** (mask bit 1 ⇒ `NotSupported` 18, fail-closed). Lý do: `TcbData` **không**
   có trường `vspace` và VSpace thuộc **arch** (D2-83: core không sở hữu PTE/PA); thêm trường chết
   chỉ để "đủ 4 tham số" sẽ tốn 512 B pin mà **không** có consumer. Cần một task riêng khi có
   đường `Resume`/`exec` tiêu thụ VSpace của TCB.
2. **`entry?` mới **lưu** + đo được, **chưa** có đường tiêu thụ**: `Tcb.Resume` (method 12) **chưa**
   cài ở kênh invocation; `exec` đọc `e_entry` từ ELF chứ không đọc `TcbData.entry`. Ghi nợ: khi
   cài `Tcb.Resume`/`WriteRegs` thì nạp `sepc` từ `entry` (và đó là chỗ kiểm căn lề có ý nghĩa
   phần cứng thật).
3. **`priority?` dùng luật MCS** (`priority ≤ mcp`) — nếu TCB có `mcp = 0` (mặc định) thì **mọi**
   priority > 0 bị từ chối; đúng ngữ nghĩa `Tcb.SetPriority` hiện có, nhưng người dùng phải
   `SetMcp` trước. Không đổi trong task này.
4. **Xung đột merge với KT-312** (cùng vòng 20): KT-312 đổi `size_of::<Machine>()`
   194.520 → 184.280 ⇒ sau merge, **bốn** dòng evidence `bytes=`/`sum=`/`static=` trong
   `ktest.sh` và phép tính trong `tls_tests.rs` phải cộng **cả hai** phần tăng (KT-311 +512,
   KT-312 −10.240) ⇒ **184.792**. Lead cần sửa lúc merge (không phải lỗi của nhánh này).
5. **`LOGDIR` dùng chung** `$TMPDIR/k3-ktest` giữa các agent chạy song song ⇒ log bị ghi đè
   (T-INFRA, §3). Đề xuất runner đặt `LOGDIR` riêng theo PID/phiên — ngoài phạm vi KT-311.
6. **Chưa chạy T2 `gate --fast`** (task yêu cầu `v2 check` + runner riscv64; T2 là việc của lead
   trên ứng viên tích hợp).
7. **Không** tuyên bố K3 đạt lại — K3 đã đóng theo D2-205.

## 8. Xác nhận luật

- **Không** push, **không** merge, **không** chạm `main` (nhánh `task/kt311-cfgfull`).
- **Không** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `v2/bin/v2`, **không** sửa
  `spec/` (D2-04) — **không** cần đổi ABI: kênh mới là `[TK]` model-level (mask `MR2` + vùng
  608..640 của IPC buffer), `[method_signature."Tcb.Configure"]` **giữ nguyên**.
- **Không** nới ngưỡng (pin `196.608` và trần `.data+.bss` `262.144` **không đổi**), **không**
  `#[ignore]`/retry, **không** `cfg(target_arch)` trong `vifast-core`/`vifast-plat-common`.
- Stage theo **đường dẫn cụ thể** (14 tệp + 1 tệp runner), **không** `git add -A`.
- Cây sạch sau commit + sau ba đột biến.
- Commit: `4e977778` (mã + runner) và `5689723a` (sửa evidence `sum=` của TAP 43).
