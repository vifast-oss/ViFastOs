# KT-290 — riscv64 **`Tcb.Configure` invocation từ U-mode**

Trạng thái: **đang làm** (nhánh `task/kt290-cfg`, base `main` @ `fe1f2ac9`), 2026-11-21.
Sub-agent `cfg`. Quyết định: **D2-190** (hợp đồng K3), **D2-205**, **D2-211**, **D2-198**
(uỷ quyền commit trên nhánh task), **D2-05** (test đỏ trước), **D2-83** (core không đọc/ghi bộ
nhớ user), **D2-104** (arity chính xác), **D2-11** (số method lấy từ spec, không chép tay).

**Không** tuyên bố K3 đạt lại: K3 đã đóng theo D2-205; KT-290 chỉ **trả một nợ K3.5 đã công bố**
(đường invocation của `Tcb.Configure`, KT-282 §6.3) và **không** mở lại bất kỳ mục nào của hợp
đồng K3.

**Ghi chú đọc trước**: `v2/agent/DECISIONS.md` **không có** mục **D2-213** (tệp dừng ở D2-212);
đã đọc D2-190, D2-205, D2-211, D2-212. Không sửa tệp DECISIONS (ngoài quyền của sub-agent).

## 1. Khảo sát trước (bắt buộc) — đường invocation từ U-mode đã có gì?

Đọc: `v2/kernel/vifast-core/src/{invoke,syscall,core_tcb,tcbdata,ipc,lib}.rs`,
`v2/kernel/vifast-plat-riscv64/src/{ipc_buffer,user,user_run}.rs`,
`v2/kernel/vifast-plat-user/lib.rs`, `v2/kernel/vifast-kbin-riscv64/src/{ktest_ipc,ktest_inv}.rs`,
`v2/kernel/vifast-kbin-riscv64/init/{ipc_client_user,ipc_server_user,inv_user,exec_a_user}.rs`,
`v2/kernel/vifast-kbin-riscv64/build.rs`, `spec/abi.toml`, `v2/lib/vifast-abi/src/generated.rs`,
`v2/test/boot/{ktest.sh,expected.txt,selftest.sh}`, `v2/agent/JOURNAL/kt282-mrbuf.md`.

| Câu hỏi | Trả lời | Bằng chứng |
|---|---|---|
| `invoke.rs` danh sách trắng những method nào? | `Untyped.Retype` (1.0), `Frame.Map` (11.0), `Frame.Unmap` (11.1), `CNode.Copy/Mint/Move/Delete/Revoke` (2.0/1/2/4/5), `Tcb.SetTls` (3.15). **`Tcb.Configure` (3.0) KHÔNG có** ⇒ rơi vào nhánh `_` ⇒ `NotSupported` **18** | `invoke.rs:324..339`, `invoke.rs:689..702` |
| `Tcb.Configure` có trong `spec/abi.toml` `[method_signature]` không? | **CÓ** — `params = ["tcb","cspace?","vspace?","entry?","ipc_buffer?","priority?"]`, `returns = []`; `[object].Tcb` method **0** = `Configure`; hằng sinh `vifast_abi::TCB_CONFIGURE = 0` | `spec/abi.toml:181,247..249`, `v2/lib/vifast-abi/src/generated.rs:245` |
| U-mode gọi invocation bằng cách nào? | syscall **0 `Call`** trên cap **không phải Endpoint**: `syscall.rs` tách nhánh ⇒ `invoke::dispatch(m, tcb, slot, args[1], &mrs, env)`; `label` = method id, `MR0..MR3` = `args`, trả `a0 = error`, `a1 = value`; **không** block/Reply | `syscall.rs:451..462`, `invoke.rs:297..340` |
| Đã có U-mode invocation thật chưa? | **CÓ** — `inv_user.rs` (KT-67a) gọi `Untyped.Retype`/`Frame.Map`/`Frame.Unmap`; `exec_a_user.rs` (KT-74) gọi `Tcb.SetTls` với method id **sinh từ spec** (`vifast_abi::TCB_SETTLS`) | `init/inv_user.rs`, `init/exec_a_user.rs:33`, `ktest_inv.rs` |
| KT-282 đã có gì ở core? | `Machine::tcb_configure_with_ipc_buffer(tcb, cspace, ipc_buffer: Option<(SlotRef, u64)>)` — bind frame 4 KiB + VA, kiểm `va % 4096`, tràn `u64`, cap `Frame` sống; **rig gọi thẳng API core** | `core_tcb.rs:152..190`, `ktest_ipc.rs:224..232` |
| Vì sao chưa nối được invocation? | (a) `Tcb.Configure` không có trong danh sách trắng; (b) `tcb_configure_with_ipc_buffer(..., None, ...)` **xoá CSpace** (`t.cspace = None`) ⇒ handler invocation không thể truyền `None` cho `cspace?`; (c) `invoke::dispatch` chỉ mang **4 MR** (`args: [u64;4]`) trong khi chữ ký có **5** tham số tuỳ chọn | `core_tcb.rs:163..186`, `invoke.rs:71,317` |

⇒ **Kết luận khảo sát**: đường invocation từ U-mode **đã có** (syscall 0 + `invoke::dispatch`);
thiếu đúng **ba** mảnh: (a) cặp `(Tcb, Configure)` trong danh sách trắng + handler; (b) API core
bind **chỉ** `ipc_buffer` mà **giữ nguyên** CSpace; (c) kênh tham số hẹp cho `ipc_buffer?` (2 word:
CPTR Frame + VA) — các tham số `cspace?`/`vspace?`/`entry?`/`priority?` **chưa cài** nên phải vắng
(arity chính xác 2, D2-104). **Không** hạ phạm vi xuống "chỉ trả error=0": TAP mới đo **cả** việc
buffer được bind **và** được dùng thật (vòng 8 MR/64 MR của KT-282 chỉ chạy được nếu buffer đã bind).

## 2. Thiết kế

### 2.1 Kênh tham số (hẹp — chỉ phần `ipc_buffer?` đã cài)

`invoke::dispatch` chỉ mang **4 MR** (`args: [u64; 4]`) trong khi
`[method_signature."Tcb.Configure"]` có **5** tham số tuỳ chọn. KT-290 **không** mở rộng kênh
`MessageInfo` (35 bit, D2-99) và **không** sửa `spec/`; nó cài **đúng một** tham số — `ipc_buffer?`
— với hai word tự nhiên của nó (cap + VA):

| MR | Ý nghĩa | Ghi chú |
|---|---|---|
| `MR0` | CPTR cap `Frame` 4 KiB trong **CSpace của caller** | `length = 2` |
| `MR1` | VA của buffer | phải căn `IPC_BUFFER_BYTES` (4 KiB), `va + 4096` không tràn `u64` |

`cspace?`/`vspace?`/`entry?`/`priority?` **chưa cài** ⇒ phải vắng; **arity chính xác 2** (D2-104):
`length` khác 2 ⇒ `InvalidArgument` **5**, có đếm. Đây là **hạ phạm vi có ghi**: mốc thật là
"U-mode `Call` trên cap TCB với `label = Tcb.Configure` trả `error = 0` **và** buffer được bind
**và** được dùng thật" — không phải "cài đủ 6 tham số".

### 2.2 Core (`vifast-core`)

| Thay đổi | Tệp |
|---|---|
| `TCB_CONFIGURE = vifast_abi::TCB_CONFIGURE` (0, sinh từ spec) | `invoke.rs` |
| `(ObjType::Tcb, TCB_CONFIGURE) => configure(...)` trong danh sách trắng + `is_whitelisted` | `invoke.rs` |
| `configure(m, tcb, slot, c, args, len)` — quyền `RIGHT_WRITE` (3), arity 2 (5), CPTR Frame trong CSpace caller (1/2/8), bind qua API core | `invoke.rs` |
| `InvocationStats.configure` + `CONFIGURED` (TAP `# ipc configure:` đối chiếu) | `invoke.rs` |
| `Machine::tcb_configure_ipc_buffer(tcb, ipc_buffer)` — bind **chỉ** `ipc_buffer?`, **giữ nguyên CSpace**, xoá `exit_code` (D2-25) | `core_tcb.rs` |
| `Machine::ipc_buffer_binding(...)` — **một** nguồn kiểm cap `Frame`/căn 4 KiB/tràn `u64` cho cả API rig (KT-282) và invocation (KT-290) | `core_tcb.rs` |
| `configure_tests.rs` — 6 test host (số method từ spec, bind + giữ CSpace + xoá `exit_code`, arity, quyền, cap/VA sai, cap/method sai) | `configure_tests.rs` |

**Vì sao cần API core mới**: `tcb_configure_with_ipc_buffer(..., cspace = None, ...)` **xoá**
CSpace (`t.cspace = None`) — đúng ngữ nghĩa "Configure không có cspace" của rig, nhưng invocation
**không** mang `cspace?` nên phải **giữ** CSpace hiện có; nếu không TCB mất CSpace và mọi syscall
sau đó trả `InvalidCapability`. API mới dùng chung hàm kiểm `ipc_buffer_binding` với API cũ ⇒
**một** ngữ nghĩa biên, không hai bản sao.

### 2.3 U-mode + rig (`vifast-kbin-riscv64`)

- `init/ipc_client_user.rs`: client **tự** gọi `Call(SLOT_TCB, {label = TCB_CONFIGURE, length = 2},
  [SLOT_FRAME, 0x4002_0000])` **trước** mọi thứ khác; in `# ipc: configure ok va=0x…` +
  `# ipc: configure err=0`; ba ca âm trong cùng boot; bind lại VA đúng; rồi mới tới vòng 8 MR/64 MR
  của KT-282. Method id lấy từ `vifast_abi::TCB_CONFIGURE` (build.rs cấp thêm `vifast_abi` cho
  ELF client — cùng khuôn `exec_a_user.rs` của KT-74).
- `src/ktest_ipc.rs`: rig **không** bind hộ IPC buffer của client nữa (`tcb_configure` chỉ cspace);
  đặt cap `Tcb` (slot 3) + cap `Frame` (slot 4) vào cspace client; server vẫn do rig bind. Đo
  **độc lập** ở kernel: `delta.configure = 3`, `delta.denied = 2`, `sys_err(0) = 0`, binding thật
  đúng VA (`# ipc configure: …`). TAP 4 = ba dòng U-mode + bốn số đo kernel + `check_ipc`/`check_caps`.
- `src/ktest_user.rs` TAP 8: ca "`Call` trên cap TCB ⇒ 18" đổi sang **method 1** (`SetFaultEp`,
  chưa cài) — từ KT-290 method 0 của Tcb là `Configure` **đã cài** nên không còn là "method ngoài
  danh sách" (arity 0 ⇒ 5). Cùng lý do, `capalloc_tests` đổi ca "`CNode.Copy` trên cap TCB ⇒ 18"
  sang cap **Endpoint** (giữ nguyên điều cần đo: cap sai loại ⇒ 18).

### 2.4 Runner (`v2/test/boot`)

- `expected.txt`: `ktest-ipc 3 3` → **`ktest-ipc 4 4`** + khối chú thích KT-290.
- `ktest.sh`: `manifest_ipc` thêm `ipc: tcb configure via invocation`; 4 evidence mới
  (`# ipc: configure ok va=`, `# ipc: configure err=0`, `# ipc: configure neg …`,
  `# ipc configure: method=0 delta=3 denied=2 bound=1 va=`); mẫu PANIC đổi sang KT-290 (4 TAP).
- `selftest.sh`: **giữ** ca (k)/(l); thêm ca **(m)** — xoá dòng `# ipc: configure ok va=…` phải đỏ
  đúng token `ev2` (ordinal tra từ `--print-evidence`, không hardcode).

## 3. Kết quả

| Việc | Kết quả |
|---|---|
| `vifast-core`: `Tcb.Configure` trong danh sách trắng + handler + API core giữ CSpace | **DONE** |
| U-mode client tự bind IPC buffer qua invocation (rig không bind hộ) | **DONE** |
| TAP 4 `ipc: tcb configure via invocation` + 3 ca âm | **DONE** |
| Runner: expected 4/4, manifest 4, 23 evidence, PANIC mới, selftest ca (m) | **DONE** |
| `cargo test -p vifast-core` | **XANH — 56/56** |
| `./v2/bin/v2 check` (cây sạch, HEAD `ce709c56` + fix `ktest_user` trong cây) | **XANH — PASS** (154.205 ms) |
| Runner riscv64 `ktest.sh 1 matrix` (HEAD `a67b4286`, rev-gate ACCEPT) | **XANH — 27/27 boot sạch** |
| `selftest.sh` | **XANH — 21/21 ca** |
| Ba đột biến | **ĐỎ đúng cả ba** (§6) |
| `.data+.bss` ảnh `ktest-ipc` | **211.464 B** / 262.144 (headroom 50.680) |
| `size_of::<Machine>()` | **không đổi** (KT-290 không thêm trường `Machine`/`TcbData`; chỉ thêm static counter + method) |

## 4. Log QEMU nguyên văn (smp=1, ảnh `ktest-ipc`, HEAD `a67b4286`)

```text
TAP version 13
1..4
# ipc rig: stvec=0x802000a0 untyped=0 core_untyped=1 cspace_c=11 cspace_s=12 tcb_c=3 tcb_s=4 ep=5 reply=6 ipc_buf=10
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
# ipc: neg empty=1 reply=18 ok=1
# ipc: neg mr over=5 ok=1
# ipc mr: round=2 cli_len=64 cli_mr4=0x28210004 cli_mr63=0x2821003f
# ipc buf: bound=1 va=0x40020000 mr_off=80 mr_words=60
ok 1 - ipc: umode call
ok 2 - ipc: server replied
ok 3 - ipc: mr4..mr7 via buffer
ok 4 - ipc: tcb configure via invocation
# ipc final: client_exit=0 server_exit=0 reply_clean=1 check_ipc=1 check_caps=1 capture_client=1 capture_server=1 client=Inactive server=Inactive
PANIC: KT-290 ipc u-mode + mr buffer + tcb configure xong (4 TAP, 0 fail)
```

Đọc dòng theo dòng (mỗi dòng là một **phép đo**, không phải lời khai):

1. `# ipc: configure ok va=0x40020000` — **chính client U-mode** in sau khi `Call` trên cap TCB
   (`label = 0`, `length = 2`) trả `a0 = 0`; VA bind đúng trang loader đã map.
2. `# ipc: configure err=0` — mã lỗi `a0` của invocation đó (client in, không phải kernel tự khai).
3. `# ipc: configure neg cap=18 method=18 outside=5 ok=1` — ba ca âm: `Call` trên cap **Reply**
   (không phải TCB) + label 0 ⇒ **18**; `Call` trên cap TCB + label **1** (chưa cài) ⇒ **18**;
   bind VA `0x4002_1000` (ngoài frame) ⇒ `Configure` 0 nhưng `Call` 8 MR ⇒ **5** (fail-closed).
4. `# ipc configure: method=0 delta=3 denied=2 bound=1 va=0x40020000` — kernel đo **độc lập**:
   3 `Tcb.Configure` được phép (bind đúng + bind ngoài frame + bind lại), 2 bị từ chối (hai ca âm
   đầu), binding **thật** của TCB client đúng VA — và rig **không** bind hộ.
5. `# ipc call: … ep_len=1 ep_mr=0x1234 …` — vòng 8 MR của KT-282 chạy **sau** khi buffer được
   bind bằng invocation ⇒ nếu invocation không chạy, dòng này và TAP 1..3 không thể xanh.
6. `ok 4 - ipc: tcb configure via invocation` — TAP mới.

## 5. TAP + matrix

```text
$ bash v2/test/boot/ktest.sh 1 matrix
# expected[ktest-ipc]: plan=4 ok=4 ten-TAP=4
REV-GATE: ACCEPT inputs=028d3f15778079c1 current_inputs=028d3f15778079c1 head_rev=a67b428686f6 count=320
ok - ktest-ipc   smp=1 boot 1 (4 TAP, 0s)   log …/ktest-ipc-smp1-1.log
ok - ktest-ipc   smp=2 boot 1 (4 TAP, 0s)   log …/ktest-ipc-smp2-1.log
ok - ktest-ipc   smp=4 boot 1 (4 TAP, 0s)   log …/ktest-ipc-smp4-1.log
# KTEST: 27/27 boot sạch (smp=matrix, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec ktest-ipc ktest-netsrv)
# ktest.sh: PASS
```

`--check-log` (23 evidence bắt buộc):

```text
$ bash v2/test/boot/ktest.sh --check-log ktest-ipc <log> 1
ok - check-log ktest-ipc smp=1 (plan=4 ok=4, 23 evidence) log <log>
# check-log: PASS
```

**Ca âm thường trực** (`bash v2/test/boot/selftest.sh`, 21/21):

```text
ok 18 - log ktest-ipc: xoá dòng '# ipc: umode call ok reply=' (evidence bắt buộc của KT-280)
#   runner đỏ vì [ev10]: not ok - check-log ktest-ipc smp=1: thiếu ev10
ok 19 - log ktest-ipc: xoá dòng '# ipc: mr count=8 mr7=' (evidence bắt buộc của KT-282)
#   runner đỏ vì [ev11]: not ok - check-log ktest-ipc smp=1: thiếu ev11
ok 20 - log ktest-ipc: xoá dòng '# ipc: configure ok va=' (evidence bắt buộc của KT-290)
#   runner đỏ vì [ev2]: not ok - check-log ktest-ipc smp=1: thiếu ev2
ok 21 - expected.txt nguyên vẹn sau mutation: sha256 fd59aa75adfb12fc… (trước = sau) + cmp
# selftest: 21/21 ca đúng như kỳ vọng
# selftest.sh: PASS
```

## 6. Đỏ trước (D2-05) + ba đột biến (đỏ có kiểm soát)

**Đỏ trước**: viết xong phía test (client + rig + runner) **trước** khi thêm nhánh dispatch; boot
ảnh `ktest-ipc`:

```text
# ipc: configure err=18
# u-mode exit: code=92 faultkind=Exit{92} tcb=3 state=Inactive
# ipc configure: method=0 delta=0 denied=1 bound=0 va=0x0
not ok 1 - ipc: umode call …
not ok 2 - ipc: server replied …
not ok 3 - ipc: mr4..mr7 via buffer …
not ok 4 - ipc: tcb configure via invocation: said_cfg=0 said_err=0 said_neg=0 delta=0 denied=1 sys_err=18 bound=0 va=0x0 client_exit=-1
PANIC: KT-290 ipc u-mode + mr buffer + tcb configure CO LOI
```

`Tcb.Configure` chưa có trong danh sách trắng ⇒ `NotSupported` **18** ⇒ client thoát 92 ⇒ **cả bốn**
TAP đỏ. Đây là hành vi **trước** KT-290, không phải lỗi cấu hình test.

Script: `/tmp/kt290-mutations.sh` (sửa **một** chỗ → build ảnh `ktest-ipc` → boot thật qua
`qemu_guard.sh` → `ktest.sh --check-log` phải đỏ → `git checkout --` trả tệp). Cây sạch sau cả ba.

| # | Đột biến | Kết quả log | Runner |
|---|---|---|---|
| M1 | **cap không phải TCB vẫn vào handler** (`(_, TCB_CONFIGURE)` thay `(ObjType::Tcb, TCB_CONFIGURE)`) | `# ipc: configure ok va=0x40020000`; ca âm (i) trả **2** (`WrongObjectType`) thay 18 ⇒ client thoát 93; `not ok 4 … sys_err=2 said_neg=0` | **ĐỎ** (rc=1) |
| M2 | **method id sai vẫn vào handler** (`(ObjType::Tcb, _)` thay `(ObjType::Tcb, TCB_CONFIGURE)`) | ca âm (ii) trả **0** thay 18 ⇒ client thoát 93; `not ok 4 … delta=2 said_neg=0` | **ĐỎ** (rc=1) |
| M3 | **bỏ kiểm dải đã map** của buffer (`read_words` không còn `user_range_mapped`) | `PANIC: trap LoadPageFault scause=0xd sepc=0x802119c4 stval=0x40021050` — đọc bừa ngoài frame | **ĐỎ** (rc=1) |

M1/M2 chứng minh **hai** ca âm "cap sai loại"/"method sai" có răng (không phải trang trí); M3
chứng minh ca "buffer ngoài frame" fail-closed **ở đúng chỗ** (arch kiểm dải đã map trước khi đọc —
D2-83: core không sở hữu PTE nên không thể tự kiểm).

## 7. Còn gì chưa xong / rủi ro

1. **Kênh tham số hẹp**: chỉ `ipc_buffer?` (2 word) được cài; `cspace?`/`vspace?`/`entry?`/
   `priority?` của `[method_signature."Tcb.Configure"]` **chưa** có đường invocation. Muốn cài đủ
   phải mở rộng kênh > 4 MR (hoặc quy ước word thứ 5..) — việc của task khác, **không** tự đặt.
2. **Không** làm (đúng phạm vi task): cap transfer, các method khác của Tcb, `Untyped.Retype`.
3. **Không** tuyên bố K3 đạt lại — K3 đã đóng theo D2-205.
4. **Rủi ro thấp**: `InvocationStats` thêm một trường `u32` (static counter, không nằm trong
   `Machine`) ⇒ pin `size_of::<Machine>()` **không đổi**; `.data+.bss` ảnh `ktest-ipc` 211.464 B
   (còn 50.680 B).
5. **Bằng chứng tái dùng** (AGENTS §6.3): `v2 check` + matrix đo trên `ce709c56`/`a67b4286`; commit
   sau chỉ thêm `JOURNAL/kt290-cfg.md` + **comment** trong `selftest.sh` (ngoài phạm vi
   build-inputs `v2/kernel v2/lib …`) ⇒ giữ nguyên bằng chứng, lý do ghi ở đây.
6. **Chưa chạy T2 `gate --fast`** (task yêu cầu `v2 check` + runner riscv64; T2 là việc của lead
   trên ứng viên tích hợp).

## 8. Xác nhận luật

- **Không** push, **không** merge, **không** chạm `main` (nhánh `task/kt290-cfg`).
- **Không** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `v2/bin/v2`, **không** sửa
  `spec/`.
- **Không** nới ngưỡng (trần `.data+.bss` 262.144 và pin `196.608` **không đổi**), **không**
  `#[ignore]`/retry, **không** `cfg(target_arch)` trong `vifast-core`/`vifast-plat-common`.
- Stage theo **đường dẫn cụ thể**, **không** `git add -A`.
- Cây sạch sau commit + sau ba đột biến (`git status --short` chỉ còn tệp của KT-290).
