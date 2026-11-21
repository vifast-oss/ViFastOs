# KT-309 — riscv64 **cap transfer qua IPC buffer** (`extra_caps`)

Trạng thái: **DONE** (nhánh `task/kt309-captransfer`, base `main` @ `00c509d5`), 2026-11-21.
Sub-agent `captransfer`. Quyết định: **D2-190** (hợp đồng K3), **D2-205**, **D2-211**, **D2-198**
(uỷ quyền commit trên nhánh task), **D2-05** (test đỏ trước), **D2-83** (core không đọc/ghi bộ
nhớ user), **D2-04/D2-22** (`spec/` đóng băng), **D2-109/D2-111** (tách ảnh khi thêm đường mới).

**Không** tuyên bố K3 đạt lại: K3 đã đóng theo D2-205; KT-309 chỉ **trả một nợ K3.5 đã công bố**
(`STATE.md` dòng `NỢ MỞ`: *"cap transfer qua IPC buffer"*) và **không** mở lại bất kỳ mục nào của
hợp đồng K3.

**Ghi chú đọc trước**: `v2/agent/DECISIONS.md` **không có** mục **D2-220** (tệp dừng ở **D2-219**);
đã đọc D2-190, D2-211, D2-219. Không sửa tệp DECISIONS (ngoài quyền của sub-agent).

## 1. Khảo sát trước (bắt buộc)

Đọc: `v2/kernel/vifast-core/src/{ipc,core_ipc,core_capxfer,core_tcb,syscall,invoke,machine,
machine_slot,tcbdata,objstate,footprint,tls_tests}.rs`,
`v2/kernel/vifast-plat-riscv64/src/{ipc_buffer,user,user_run,core_bridge}.rs`,
`v2/kernel/vifast-plat-user/lib.rs`, `v2/kernel/vifast-kbin-riscv64/src/ktest_ipc.rs`,
`v2/kernel/vifast-kbin-riscv64/init/{ipc_client_user,ipc_server_user}.rs`, `spec/abi.toml`,
`v2/agent/JOURNAL/{kt282-mrbuf,kt290-cfg,kt67b-journal,kt63-nhat-ky}.md`,
`v2/test/boot/{ktest.sh,expected.txt,selftest.sh}`.

| Câu hỏi | Trả lời | Bằng chứng |
|---|---|---|
| `extra_caps` nằm ở **bit nào** của `MessageInfo`? | Trường tên đầy đủ là **`extra_caps_unwrapped`**, **bit 24..27** (4 bit) của thanh ghi 35 bit; `layout = "label:0..19, caps:20..22, no_reply:23, extra_caps_unwrapped:24..27, length:28..34"` | `spec/abi.toml:44-48`, `ipc.rs:86,200` |
| `ipc.rs` **từ chối** ở đâu? | **KHÔNG** từ chối `extra_caps_unwrapped > 0`. `MessageInfo::new` chỉ từ chối `extra_caps_unwrapped >= 16` (biên 4 bit) — **giá trị 1..15 hợp lệ**. Tiền đề của phiếu (*"`ipc.rs` từ chối `extra_caps_unwrapped > 0`"*) **không đúng** với HEAD `00c509d5` | `ipc.rs:122-124`, `ipc.rs:169-175` |
| Vậy `extra_caps_unwrapped` đang **dùng** cho gì? | **Đã cài** từ KT-63: nó là **số cap đã unwrap** (badge OR vào message), do `deliver_caps` ghi (`with_extra_caps_unwrapped(plan.unwrapped())`). Sim pin ngữ nghĩa này | `core_capxfer.rs:119`, `v2/sim/vifast-sim/tests/kt25_ipc2.rs:486` (`"một cap unwrap"`), `kt63_core_cap_ipc.rs:249,280` |
| IPC buffer có chỗ cho **danh sách cap** không? | **CÓ, hai chỗ**: (a) **header** 10 word đã có **4 CPTR nguồn** (`send_caps`, word 6..9) — đường KT-67b cho **≤ 4** cap; (b) vùng **dự phòng** `byte 560..4096` **trống hoàn toàn** (KT-282 chỉ dùng `80..560` cho MR4..MR63) | `ipc.rs:578-586`, `kt282-mrbuf.md:35-42` |
| `CNode` có hàm `copy`/`mint` **dùng lại được** không? | **CÓ** — `Machine::copy_rights(src, dst, rights)` / `mint(src, dst, rights, badge)` / `derive(...)`; `derive` **đã** kiểm `check_cnode_slot` + `SlotOccupied` (7) và tạo cap con qua `push_cap`. Không cần viết lại | `machine.rs:892-975` |
| Quyền: caller phải có gì trên cap nguồn? | `RIGHT_GRANT` (bit 2, `spec` `[rights]`); `plan_cap_transfer` của KT-63 coi thiếu `RIGHT_GRANT` là **thất bại mềm**; cap cài sang bên nhận **bỏ** `RIGHT_GRANT` | `spec.rs:28`, `ipc.rs:322-330`, `core_capxfer.rs:104` |
| Quyền: receiver phải có gì trên CNode đích? | Model **không** có cap-quyền cho **cspace gốc** của một TCB (`TcbData.cspace: Option<ObjId>`, không phải `SlotRef` + quyền) ⇒ kiểm được **CNode còn sống** + **slot trong span** + **slot rỗng**; phần "`RIGHT_WRITE` trên CNode đích" **không biểu diễn được** trong model hiện tại (ghi nợ §7) | `tcbdata.rs:24`, `machine_slot.rs:12,68,88` |
| `Message` mang được cap **qua hàng chờ** không? | **CÓ** — `Message.caps: [CapId; 4]` + `snaps: [CapSnapshot; 4]` + `n_caps`/`n_snaps`; `resolve_caps` chụp ảnh lúc `admit`, `deliver_caps` cài lúc giao. Pool `ipc_msgs: [QueuedMsg; 24]` | `ipc.rs:400-470`, `core_capxfer.rs:20-40`, `machine.rs:62,220` |
| Ngân sách tĩnh còn bao nhiêu? | `size_of::<Machine>()` = **190.920 B** / pin **196.608** (headroom **5.688**); ảnh chính `.data+.bss` **246.936** / 262.144 | `tls_tests.rs:332-360`, `ktest.sh:588,601,631,791` |

⇒ **Kết luận khảo sát**: (a) `extra_caps_unwrapped` **đã có chủ** (unwrap badge, KT-63) — dùng nó
làm **số cap** là **đổi ngữ nghĩa ABI** (tên trường trong `spec` nói "unwrapped", sim pin ngữ
nghĩa unwrap) ⇒ **không làm**; (b) đường **≤ 4 cap** đã có (KT-67b, header `send_caps`); (c) chỗ
**trống** duy nhất cho danh sách cap **> 4** là vùng dự phòng `560..4096` của IPC buffer — **cùng
khuôn `[TK]`** mà KT-282 đã dùng cho MR4..MR63 (*"spec không chốt offset vùng MR"*).

## 2. Thiết kế (kênh hẹp — ghi rõ)

### 2.1 Vì sao **không** dùng `MessageInfo.extra_caps_unwrapped`

Phiếu ghi *"`extra_caps = n`"* như trường của `MessageInfo`. Nhưng trường 4 bit duy nhất ở
`24..27` là **`extra_caps_unwrapped`** và **đã được KT-63 cài** làm **số cap unwrap** (badge OR),
có test sim pin (`kt25_ipc2.rs:486`, `kt63_core_cap_ipc.rs:249,280`) và tên trường trong `spec`
nói rõ "unwrapped". Gán lại nó thành "số cap trong IPC buffer" là **đổi ngữ nghĩa ABI** ⇒ theo
luật của phiếu (*"nếu cần đổi ABI thì DỪNG, báo lead"*) **không tự làm**; thay vào đó **số `n`
nằm trong chính vùng dự phòng của IPC buffer** — đúng khuôn `[TK]` của KT-282/KT-290, **không**
chạm `spec/`, **không** đổi `MessageInfo` 35 bit. Ghi thành **nợ/đề xuất** ở §7 để lead quyết.

### 2.2 Layout IPC buffer (`[TK]` KT-309 — nối tiếp KT-282)

```text
byte   0.. 80 : header 10 word (magic, MessageInfo bản ghi, receive window 4 CPTR, send caps 4 CPTR) — KT-67b
byte  80..560 : MR4..MR63 (60 word = 480 B) — KT-282
byte 560..568 : **extra_caps count n** (0..=4) — KT-309
byte 568..600 : **n CPTR nguồn** (4 × 8 B, CPTR trong CSpace **caller**) — KT-309
byte 600..608 : **extra_caps dst** (slot đích trong CSpace **receiver**; 0 = "slot trống đầu tiên") — KT-309
byte 608..4096: dự phòng (zero)
```

Hằng **một nguồn** ở `vifast_core::ipc` (`IPC_BUFFER_EXTRA_CAPS_OFFSET` = `IPC_BUFFER_MR_END` =
560, `IPC_BUFFER_EXTRA_CAPS_WORDS` = 5, `IPC_BUFFER_EXTRA_CAPS_DST_OFFSET` = 600,
`IPC_BUFFER_EXTRA_CAPS_END` = 608); shim U-mode dùng **cùng** số. **Không** đổi `spec/`.

### 2.3 Ngữ nghĩa

**Gửi** (`build_message`, caller đang chạy ⇒ arch đọc bộ nhớ user, D2-83):

1. `n = words[0]`; `n > 4` ⇒ **`InvalidArgument` (5)** — biên `[message].max_caps = 4`, **không**
   nới (đột biến **M2**).
2. Mỗi CPTR `i < n`: `cap_slot` trong CSpace **caller**; cap phải **sống** (chết/rỗng ⇒
   `InvalidCapability` 1).
3. Cap nguồn **thiếu `RIGHT_GRANT`** ⇒ **`InsufficientRights` (3)** — lỗi **cứng** ngay ở đường
   gửi (khác KT-63 "mềm" cho cap trong thanh ghi: ở đây caller **chủ động** khai `n` nên sai là
   lỗi) (đột biến **M1**).
4. Chụp ảnh (`CapSnapshot`) vào `Message.extra_snaps` — message **không** copy object, chỉ cap
   tham chiếu (cùng nguyên tắc KT-63).

**Nhận** (`deliver_extra_caps`, gọi từ `deliver_message` sau `deliver_caps`):

5. Slot đích: `dst` do **receiver** khai ở `byte 600` (đọc lúc `Recv` — receiver đang chạy);
   `dst == 0` ⇒ **slot trống đầu tiên** trong CSpace receiver (quét từ 1). Cap thứ `i` vào
   `dst + i`.
6. Slot đích **đã có cap** ⇒ **`SlotOccupied` (7)** — lỗi **cứng**, message **không** giao
   (đột biến **M3**).
7. Cap cài sang receiver **bỏ `RIGHT_GRANT`**, **giữ badge** nguồn (cùng luật KT-63/thiết kế §4.6).
8. Cap nguồn chết giữa lúc xếp hàng và lúc giao ⇒ **thất bại mềm** cho **riêng** cap đó
   (`cap_transfer_failed` tăng), message **vẫn** giao — cùng chuẩn KT-63.

### 2.4 Đo

`# ipc caps: sent=<n> received=<n> slot=<n> rights=0x…` — `sent` = số cap client khai, `received`
= số cap **thật** cài được vào CSpace server (kernel đếm), `slot` = slot đầu tiên, `rights` =
quyền **thật** của cap trong CSpace server (đọc lại từ bảng cap, **không** tin lời chương trình).
TAP mới: `ok 5 - ipc: cap transfer`.

### 2.5 Ca âm (trong cùng boot)

| # | Ca | Kỳ vọng |
|---|---|---|
| (i) | cap nguồn **thiếu `RIGHT_GRANT`** | `InsufficientRights` **3** |
| (ii) | `n = 5` (> `max_caps = 4`) | `InvalidArgument` **5** |
| (iii) | slot đích **đã bận** (receiver khai `dst` = ô đang có cap) | `SlotOccupied` **7** |

Giữ nguyên **toàn bộ** ca âm cũ của KT-280/KT-282/KT-290.

## 3. Kết quả

| Việc | Kết quả |
|---|---|
| `vifast-core`: vùng extra caps + `Message.extra_snaps` + `build_message` kiểm `n`/cap sống/`RIGHT_GRANT` | **DONE** |
| `vifast-core`: `deliver_extra_caps` + `extra_caps_slots` (kiểm slot **trước** khi pop) | **DONE** |
| `vifast-plat-riscv64`: `read_header` đọc **thêm** 6 word vùng extra caps (cùng trang, cùng magic) | **DONE** |
| `vifast-plat-user`: `set_extra_caps`/`extra_caps`/`extra_cap`/`set_extra_caps_dst`/`extra_caps_dst` | **DONE** |
| Ảnh `ktest-ipc`: rig 2 cap `Frame` (có/thiếu `GRANT`) + 8 lượt + TAP 5 + 3 ca âm | **DONE** — `.data+.bss` = **215.064 B** / 262.144 (headroom 47.080) |
| Runner: `expected.txt` 4→5, `manifest_ipc` 5, **28** evidence, mẫu PANIC mới, `selftest.sh` ca (o) | **DONE** |
| `cargo test -p vifast-core` | **XANH — 61/61** (5 test mới cho KT-309) |
| `./v2/bin/v2 check` | **XANH** (exit 0, `rules: PASS`) |
| Runner riscv64 `ktest.sh 1 matrix` (cây **sạch**, HEAD `7f82294f`) | **XANH — 27/27 boot sạch** |
| `selftest.sh` | **XANH — 23/23 ca** (ca (o) mới: `ev23`) |
| Ba đột biến | **ĐỎ đúng cả ba** (§6) |
| `size_of::<Machine>()` | 190.920 → **194.520 B** (pin **196.608 không đổi**, headroom 2.088) |

**Ghi chú bắt buộc — `size_of::<Machine>()` 190.920 → 194.520 B** (+3.600): `TcbData` thêm
`Option<u16>` (`extra_caps_dst`, đệm căn 8 ⇒ 8 B × 64 = **512**), `Message` thêm
`extra_snaps[4]` + `n_extra` (đệm căn 8 ⇒ **128 B** × `MAX_QUEUED_MESSAGES` 24 = **3.072**), hai
bộ đếm `u64` (**16**). Pin **196.608 không đổi**; `tls_tests::pin_machine_tang_dung_8_byte_moi_tcb`
cập nhật **phép tính** (không nới ngưỡng) và 5 dòng evidence `bytes=`/`static=` trong `ktest.sh`
đổi theo **số đo thật** (`190920` → `194520`) — cùng cách KT-282 đã làm.

## 4. Log QEMU nguyên văn (smp=1, ảnh `ktest-ipc`, HEAD `7f82294f`)

```text
TAP version 13
1..5
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
# ipc mr: round=2 cli_len=64 cli_mr4=0x28210004 cli_mr63=0x2821003f
# ipc: cap busy=7 ok=1
# ipc: cap used slot=5 copy=6
# ipc caps: sent=1 received=1 slot=5 rights=0xb obj=10 ty=Frame alive=1 copy_slot=6 copy_obj=10 copy_ty=Frame same=1
# ipc: cap sent=1 slot=5
# ipc: cap neg grant=3 over=5 ok=1
# ipc: neg empty=1 reply=18 ok=1
# ipc: neg mr over=5 ok=1
# ipc buf: bound=1 va=0x40020000 mr_off=80 mr_words=60
ok 1 - ipc: umode call
ok 2 - ipc: server replied
ok 3 - ipc: mr4..mr7 via buffer
ok 4 - ipc: tcb configure via invocation
ok 5 - ipc: cap transfer
# ipc final: client_exit=0 server_exit=0 reply_clean=1 check_ipc=1 check_caps=1 capture_client=1 capture_server=1 client=Inactive server=Inactive
PANIC: KT-309 ipc u-mode + mr buffer + tcb configure + cap transfer xong (5 TAP, 0 fail)
```

Đọc dòng theo dòng (mỗi dòng là một **phép đo**, không phải lời khai):

1. `# ipc: cap busy=7 ok=1` — **chính server U-mode** in: nó khai `dst` = ô **1** (đang có cap
   `Endpoint`) rồi `Recv` ⇒ kernel trả **`SlotOccupied` (7)** **trước** khi pop ⇒ message **không**
   mất.
2. `# ipc: cap used slot=5 copy=6` — server khai lại `dst` = ô **5** (trống), `Recv` lần hai nhận
   message + cap, rồi **dùng** cap: `CNode.Copy` ô 5 → ô 6 thành công (cap **thật**, sống, đúng
   loại — không phải "cap giả").
3. `# ipc caps: sent=1 received=1 slot=5 rights=0xb obj=10 ty=Frame alive=1 copy_slot=6
   copy_obj=10 copy_ty=Frame same=1` — **kernel đo độc lập** (không tin lời chương trình):
   `extra_caps_sent` = 1, `extra_caps_installed` = 1, cap trong CSpace server ở ô 5 là `Frame`
   **sống**, quyền `0xb` = `READ|WRITE|GRANT_REPLY` (**đã bỏ `RIGHT_GRANT`** — nguồn có `0xf`),
   và bản `CNode.Copy` ở ô 6 trỏ **cùng** object `10` ⇒ server **dùng được** cap.
4. `# ipc: cap sent=1 slot=5` — **chính client U-mode** in sau khi `Call` mang extra cap trả 0.
5. `# ipc: cap neg grant=3 over=5 ok=1` — **chính client** in hai ca âm: cap nguồn **thiếu
   `RIGHT_GRANT`** ⇒ **3** (`InsufficientRights`); `n = 5` > `max_caps = 4` ⇒ **5**
   (`InvalidArgument`).
6. `ok 5 - ipc: cap transfer` — TAP mới.

## 5. TAP + matrix

```text
$ bash v2/test/boot/ktest.sh 1 matrix
# expected[ktest-ipc]: plan=5 ok=5 ten-TAP=5
# image=ktest-ipc … data+bss=215064 budget=262144
ok - ktest-ipc   smp=1 boot 1 (5 TAP, 0s)   log …/ktest-ipc-smp1-1.log
ok - ktest-ipc   smp=2 boot 1 (5 TAP, 1s)   log …/ktest-ipc-smp2-1.log
ok - ktest-ipc   smp=4 boot 1 (5 TAP, 0s)   log …/ktest-ipc-smp4-1.log
# KTEST: 27/27 boot sạch (smp=matrix, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec ktest-ipc ktest-netsrv)
# ktest.sh: PASS
```

`--check-log` (**28** evidence):

```text
$ bash v2/test/boot/ktest.sh --check-log ktest-ipc <log> 1
ok - check-log ktest-ipc smp=1 (plan=5 ok=5, 28 evidence) log <log>
# check-log: PASS
```

**Ca âm thường trực** (`bash v2/test/boot/selftest.sh`, **23/23**):

```text
ok 21 - log ktest-ipc: xoá dòng '# ipc caps: sent=1 received=1 slot=5 rights=0x' (evidence bắt buộc của KT-309)
#   runner đỏ vì [ev23]: not ok - check-log ktest-ipc smp=1: thiếu ev23
ok 23 - expected.txt nguyên vẹn sau mutation: sha256 496f664b2de0f3a8… (trước = sau) + cmp
# selftest: 23/23 ca đúng như kỳ vọng
# selftest.sh: PASS
```

## 6. Ba đột biến (đỏ có kiểm soát) — cả ba ĐỎ đúng chỗ

Script: `/tmp/kt309-mutations.sh` (sửa **một** chỗ → build ảnh `ktest-ipc` → boot thật qua
`qemu_guard.sh` → `ktest.sh --check-log` phải đỏ → `git checkout --` trả tệp). Cây sạch sau cả ba.

| # | Đột biến | Kết quả log | Runner |
|---|---|---|---|
| **M1** | bỏ kiểm `RIGHT_GRANT` của cap nguồn | `# ipc: cap neg grant err=0` (đáng lẽ 3); client thoát **95** | **ĐỎ**: `not ok 5 … client_exit=95` + thiếu `ev17/ev18` |
| **M2** | bỏ kiểm `n > max_caps = 4` | `# ipc caps: sent=4 received=4` (n=5 bị **cắt** còn 4 — `min` phòng thủ, **không** panic); `# ipc: cap copy err=7`; server thoát **63** | **ĐỎ**: `not ok 5 … sent=4 received=4 server_exit=63` |
| **M3** | bỏ kiểm slot đích **bận** | `# ipc: cap busy err=0` (đáng lẽ 7); server thoát **60**; `check_caps=0` | **ĐỎ**: `not ok 5 … said_busy=0 server_exit=60` |

M1 chứng minh ca âm (i) có răng (không phải trang trí); M2 chứng minh biên `max_caps = 4` **không**
nới; M3 chứng minh `SlotOccupied` **trước** khi pop là thật (bỏ kiểm ⇒ cap cài **đè** ô 1 và
`check_caps` đỏ).

## 7. Còn gì chưa xong / rủi ro

1. **Tiền đề của phiếu không đúng** (đã báo ở §1/§2.1): `ipc.rs` **không** từ chối
   `extra_caps_unwrapped > 0`; trường 4 bit `24..27` **đã có chủ** (số cap **unwrap**, KT-63, sim
   pin). Vì vậy **số `n`** của extra cap nằm trong **vùng dự phòng của IPC buffer** (offset 560),
   **không** phải `MessageInfo` — **không** đổi ABI, **không** chạm `spec/`. **Đề xuất cho lead**:
   nếu muốn `extra_caps` thành trường ABI thật thì phải **mở `spec/`** (thêm trường/đổi nghĩa
   `extra_caps_unwrapped`) — việc của một task có quyền sửa spec, **không** phải task này.
2. **`RIGHT_WRITE` trên CNode đích của receiver KHÔNG kiểm được**: model không có cap-quyền cho
   **cspace gốc** của TCB (`TcbData.cspace: Option<ObjId>`, không phải `SlotRef` + quyền). Đã kiểm
   được: CNode **còn sống**, slot **trong span**, slot **rỗng** (`SlotOccupied`). Muốn kiểm đủ phải
   đổi model CSpace (task khác).
3. **Server "dùng" cap bằng `CNode.Copy`**, **chưa** `Frame.Map`: cspace của server trong ảnh này
   **không** có cap `VSpace` (rig chỉ cấp `CNode`/`Endpoint`/`Reply`/`Tcb`/`Frame`). `Frame.Map`
   bằng cap nhận được đã có bằng chứng ở **TAP 19 của `ktest-user`** (KT-67b, `ktest_capx.rs`:
   map/ghi/đọc 8 B thật) — thêm vào ảnh này là việc mở rộng, **không** phải mốc của KT-309.
4. **Không** làm (đúng phạm vi task): cap **badge** qua buffer, `Revoke` xuyên cap, cap cho
   Endpoint, nhiều vùng extra cap/TCB, scatter-gather.
5. **Rủi ro thấp**: `size_of::<Machine>()` 194.520 / pin 196.608 (headroom **2.088** — mỏng hơn
   trước, ghi để task sau biết); `.data+.bss` ảnh `ktest-ipc` 215.064 / 262.144.
6. **Chưa chạy T2 `gate --fast`** (task yêu cầu `v2 check` + runner riscv64; T2 là việc của lead
   trên ứng viên tích hợp).
7. **Không** tuyên bố K3 đạt lại — K3 đã đóng theo D2-205.

## 8. Xác nhận luật

- **Không** push, **không** merge, **không** chạm `main` (nhánh `task/kt309-captransfer`).
- **Không** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `v2/bin/v2`, **không** sửa
  `spec/`.
- **Không** nới ngưỡng (pin `196.608` và trần `.data+.bss` `262.144` **không đổi**), **không**
  `#[ignore]`/retry, **không** `cfg(target_arch)` trong `vifast-core`/`vifast-plat-common`.
- Stage theo **đường dẫn cụ thể** (19 tệp), **không** `git add -A`.
- Cây sạch sau commit + sau ba đột biến.
