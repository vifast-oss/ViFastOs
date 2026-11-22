# KT-342b — đóng **hai nợ** của `KT-342`: tách `plat-riscv64/user.rs` theo miền (1.499 → **704** dòng) · sửa **gốc rễ** lỗi `NBSend` mất message ⇒ `netsrv` trả lời `shell` **trực tiếp** U-mode→U-mode

**Chu kỳ 2 — Đợt nước rút cuối.** Sub-agent `rv342b`; nhánh `task/kt342b`, worktree `/tmp/kt342b-s1`,
base `main` @ `387b8f9e`. Ngày 2026-11-23 (giờ máy Asia/Tokyo).
Quyết định: **D2-250** (mở `KT-342b`, đóng 2 nợ `kt342-riscv.md` §9), **D2-247/D2-245** (quy chế
sub-agent: **chỉ T0+T1**, journal **một** tệp), **D2-243** (tách `user.rs` **theo miền**, trần R1
1.500 dòng/tệp, mục tiêu `user.rs < 1.100`), **D2-79/D2-68/D2-77** (ngữ nghĩa lỗi/`post`/hộp thư),
**D2-05** (đỏ trước), **D2-17** (`unsafe` chỉ trong `vifast-plat-*`), **D2-08** (0 harness marker).

| | |
|---|---|
| Worktree | `/private/tmp/kt342b-s1` |
| Base | `387b8f9e` (`main`, gồm `KT-342` `55d35b49`) |
| Máy | macOS Apple Silicon, rustc/cargo `1.100.0`, QEMU `11.1.3` |
| Lệnh | `export PATH="$HOME/.cargo/bin:$PATH"`, chạy trong worktree |
| Nhãn số liệu | **`[ĐO KT-342b]`** — đo từ log runner/host test trong worktree này |

Phạm vi: `v2/kernel/vifast-core` (sửa gốc rễ IPC + test host + 2 đột biến),
`v2/sim/vifast-sim/src/kobj/tcb.rs` (parity oracle), `v2/kernel/vifast-plat-riscv64/src/user{,/*}.rs`
(tách theo miền), `v2/kernel/vifast-{kbin,plat-user}-riscv64` (đường trả lời trực tiếp),
`v2/test/boot/product-riscv64.sh` (thêm 2 phép kiểm đường trực tiếp), tệp journal này.
**Không** chạm: `spec/`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/**`, `.github/**`, 19 tệp
`[[legacy]]`, `main`.

---

## 1. Nợ (1) — tách `user.rs` theo **miền**: **1.499 → 704 dòng** (mục tiêu < 1.100)

### 1.1 Ba miền tách ra (chuyển **nguyên văn**, 0 đổi logic)

| Tệp mới | Miền | Dòng | Nội dung chuyển ra |
|---|---|---:|---|
| `src/user/load.rs` | **nạp ảnh ELF** | **313** | `LoadError` + `name()`, `load_init`/`load_init_slot`/`load_init_slot_with_stack` (parse `vifast-elf`, map segment theo quyền, stack user + guard, IPC buffer, `wx_pages`/`flags_ok` đi lại PTE thật), `write_page`, `USER_STACK_PAGES` |
| `src/user/trap.rs` | **trap U-mode** | **316** | `on_u_exception` (syscall `scause=8` → `abi::dispatch` trên stack riêng; fault U-mode → `classify`/pager/PANIC), `classify_u_fault` (PTE thật khi `ktest`), `read_sstatus`, `exit_to_kernel` |
| `src/user/mem.rs` | **bộ nhớ user / `SyscallEnv`** | **215** | `UserMemAccess` + toàn bộ `impl SyscallEnv` (`put_str` qua `sstatus.SUM`, `resolve_addr`, `now_ns`, `invoke`, `release_object`, `ipc_header`/`ipc_buffer_words`, `tcb_resume`/`tcb_vspace`), `user_range_mapped`, `with_kernel_root` |

`user.rs`: **1.499 → 704** dòng (`-795`), vẫn giữ: hằng cửa sổ user/stack, bảng dải theo ô tác vụ,
console capture (KT-65), `USER_TASK`, gắn TCB core + sổ syscall, `enter*`/`enter_with`,
`dispatch_on_private_stack` (+ asm `user_enter`/`user_exit_resume`) — tức phần **trạng thái + cửa
vào/ra** của runtime U-mode.

### 1.2 Giữ **nguyên** API công khai (grep trước khi cắt, không phá ai)

`user.rs` tái xuất đúng đường dẫn cũ nên **0 tệp khác phải sửa**:

```rust
pub mod load; pub mod mem; pub mod trap;
pub use load::{load_init, load_init_slot, load_init_slot_with_stack, LoadError, USER_STACK_PAGES};
pub(crate) use load::write_page;                 // init_pages.rs dùng
pub(crate) use mem::{user_range_mapped, UserMemAccess};
pub use trap::on_u_exception;
pub(crate) use trap::read_sstatus;               // user_tls.rs dùng
```

Mỗi mô-đun con `use super::*;` ⇒ **thấy** hằng/trạng thái private của `user.rs` đúng như khi còn
cùng tệp (không phải nới `pub(crate)` hàng loạt, không đổi ngữ nghĩa). Kiểm bằng máy:
`cargo check -p vifast-plat-riscv64 --features ktest` **0 error, 0 warning mã** (chỉ 4 warning
`unused dependency` **có sẵn** của manifest: `vifast-elf`/`vifast-net`/`vifast-ring`/`vifast-virtio`
chỉ dùng dưới feature khác); luật cấu trúc R1..R5 của `v2 check` **PASS** trên cả 4 tệp.

### 1.3 Không đổi hành vi — bằng chứng

Boot thật sau khi tách: `# shell: up cmds=…` (18 lệnh), `# product fs: medium=virtio-blk`,
`# shell: ls/write/cat`, `# product net: served=1 …`, `# shell: ping 10.0.2.2 rtt_ms=6 err=0`,
`# product exec: path=/hello.elf … check=1`, `RUN OK`, `# product halt: shell_exit=0`
(xem §4.3). `size_of::<Machine>()` **không đổi** (§4.2).

---

## 2. Nợ (2) — lỗi `NBSend` **mất message**: tái hiện → gốc rễ → sửa → chứng minh

### 2.1 Tái hiện trên QEMU (**đỏ-trước** — trước khi sửa)

Đổi `netsrv` trả lời `shell` bằng `NBSend` trên `EP_NET_REPLY` (rồi `Exit`) và cho `shell` poll
`NBRecv` ô đó — đường "hai U-mode nói chuyện trực tiếp" mà `KT-342` đã bỏ:

```text
# product net: served=1 denied=0 failed=0 err=0 rtt_ms=5 sent=1 received=1 label=0x291 caller=26
# netsrv: ping 10.0.2.2 rtt_ms=5 err=0
# netsrv: reply nb err=0          ← NBSend TRẢ 0 (thành công)
# shell: ping timeout             ← nhưng shell KHÔNG BAO GIỜ nhận được ⇒ MẤT message
```

Chẩn đoán bằng bộ đếm **của Endpoint** (thêm tạm `endpoint_info` vào vòng `proc_rt`, đã gỡ trước
commit) — cùng **một** object `Endpoint` cho cả hai bên (`root15_obj=22 cnode2_obj=Some(22)`),
`shell` poll đúng ô, và:

```text
# diag42b: rep(obj=22 len=0 push=1 pop=0 rm=1 waiter=None) … last_mr=Some(6)
```

⇒ message **đã vào hàng** (`push=1`), **không ai lấy** (`pop=0`) mà bị **gỡ** (`rm=1`). Đây không
phải lỗi "không xếp hàng" như giả thuyết trong `kt342-riscv.md` §9.4.

### 2.2 Gốc rễ: TCB gửi `Exit` **rút lại** message `NBSend` đã trả về thành công

`Machine::exit`/`finalise_tcb` gọi `unlink_tcb_queues`; bản cũ gỡ **mọi** message có
`sender == tcb`:

```rust
// TRƯỚC (vifast-core/src/core_ipc.rs, unlink_tcb_queues)
if q.sender == tcb { if q.msg.is_some() { msg_dropped += 1; } self.free_ipc_msg(mi); }
```

Kèm theo, `check_ipc` đòi **sender còn sống** cho *mọi* message đang xếp hàng — bất biến này
**buộc** phải huỷ message `NBSend` của TCB đã chết (nếu giữ, `check_ipc` đỏ ngay sau `Exit`).

Ngữ nghĩa đúng (và là thứ `netsrv` cần): `NBSend` **đã trả 0** ⇒ message **thuộc về bên nhận**;
cái chết của người gửi **không** được rút lại. Chỉ message **chặn** (`QueuedMsg.blocking`:
`Send`/`Call`/fault đang `BlockedOnSend`) mới thuộc về TCB gửi — giao nó sẽ phải `wake`/chuyển
trạng thái một TCB đã chết — nên vẫn gỡ (và đếm `ipc_dropped`) như cũ.

### 2.3 Sửa (3 chỗ, tối thiểu, cùng một luật)

1. `vifast-core/src/core_ipc.rs` — `unlink_tcb_queues` thành **hai** biến thể:
   `unlink_tcb_queues(tcb)` (giữ **nguyên** luật cũ: gỡ **mọi** message; dùng cho mọi đường
   **chuyển trạng thái**) và **`unlink_tcb_queues_keep_committed(tcb)`** (chỉ gỡ message
   **chặn** `blocking`; message `NBSend` đã trả `Ok` **ở lại** hàng) — dùng đúng **3 chỗ của
   `Machine::exit`** và **1 chỗ của `finalise_tcb`** (đường TCB **chết**).
2. `vifast-core/src/check_ipc.rs` — `object_alive(q.sender)` chỉ áp cho `q.blocking`; message
   `NBSend` **được phép** có sender đã chết (giữ nguyên vế `blocking ⇒ sender BlockedOnSend{ep,call,fault}`).
3. `v2/sim/vifast-sim/src/kobj/{tcb,fault,object}.rs` (oracle) — **cùng** luật hai biến thể
   (`unlink_tcb_queues_keep_committed` cho `Exit`/finalise) ⇒ **parity** core↔sim giữ nguyên;
   `cargo test -p vifast-sim` xanh (§4.1).

**Vì sao phải thu hẹp như vậy (đo được, không phải suy đoán)**: bản đầu tôi đổi luật cho **mọi**
đường `unlink_tcb_queues` ⇒ **`ktest-user` 24/27 → đỏ `not ok 9`** (`matrix 37/37 → 13/37`).
Chẩn đoán trong ảnh test (in `endpoint_info`/`endpoint_send_items` ở TAP 9, đã gỡ): hàng
`EP_SVC` còn **1** message `sender=3 (=T0) msg_id=6` từ TAP 7 (sender **còn sống**) ⇒ `Recv` của
T1 **pop** message cũ thay vì đăng ký waiter ⇒ `Call` của T0 rơi vào nhánh "không waiter"
(`blocked=0`, `caller=BlockedOnSend`). Tức 9 ảnh test **dựa** vào luật "chuyển trạng thái thì gỡ
message của TCB" (bất biến §4.4) — nới nó là **đổi ngữ nghĩa toàn cục**, không phải việc của
task 2 h. Vì vậy bản chốt **chỉ** mở cho đường TCB **chết**, đúng đường sản phẩm cần.

Không thêm trường nào vào `Machine` ⇒ `size_of::<Machine>()` không đổi (§4.2). Ngữ nghĩa này là
**làm rõ** hợp đồng `NBSend` (message đã nhận thì không bị rút) — đề nghị Agent 0 ghi thành mục
`D2-*` khi duyệt (tôi **không** sửa `DECISIONS.md`).

**Nợ còn lại (nói thẳng, đề xuất task con)**: luật "**chuyển trạng thái** thì gỡ **mọi** message
của TCB" vẫn giữ nguyên (`unlink_tcb_queues` cũ) nên một `NBSend` **vẫn có thể mất** nếu người
gửi **đổi trạng thái** (Yield/block/suspend) trước khi bên nhận kịp `Recv` — chỉ đường **chết**
được sửa. Mở rộng `keep_committed` cho toàn bộ đường chuyển trạng thái đòi sửa bất biến §4.4 +
9 ảnh test (mỗi ảnh phải drain/khẳng định lại) ⇒ **task con đề xuất `KT-342c`** (2–3 h, D2-05:
ca đỏ-trước là TAP 9 hiện có + host test `nb_send_song_sot_qua_exit_cua_sender` mở rộng sang
`Yield`/`Tcb.Suspend`).

### 2.4 Ca **đỏ-trước** (host test, D2-05): `v2/kernel/vifast-core/src/nb_queue_tests.rs`

Bốn ca: (1) `NBSend` không waiter ⇒ vào hàng, `NBRecv` lấy đúng MR; (2) **ca đỏ-trước** `NBSend`
rồi sender `Exit` ⇒ message **vẫn** ở hàng và vẫn nhận được; (3) `Send` **chặn** + sender chết ⇒
gỡ **có đếm** `ipc_dropped` (ngữ nghĩa cũ, không đổi); (4) thứ tự lỗi: cap trống **trước** trạng
thái TCB ⇒ `InvalidCapability` 1.

**Trước khi sửa** (`cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-core nb_queue`):

```text
test nb_queue_tests::blocking_send_cua_sender_chet_bi_go ... ok
test nb_queue_tests::loi_cap_dung_truoc_loi_trang_thai ... ok
test nb_queue_tests::nb_send_khong_waiter_vao_hang ... ok
test nb_queue_tests::nb_send_song_sot_qua_exit_cua_sender ... FAILED
thread '…nb_send_song_sot_qua_exit_cua_sender' panicked:
  assertion `left == right` failed: message `NBSend` đã trả thành công KHÔNG được rút lại khi sender Exit
  left: 0
 right: 1
test result: FAILED. 3 passed; 1 failed
```

**Sau khi sửa**:

```text
test nb_queue_tests::blocking_send_cua_sender_chet_bi_go ... ok
test nb_queue_tests::loi_cap_dung_truoc_loi_trang_thai ... ok
test nb_queue_tests::nb_send_khong_waiter_vao_hang ... ok
test nb_queue_tests::nb_send_song_sot_qua_exit_cua_sender ... ok
test result: ok. 4 passed; 0 failed
```

Toàn bộ host test core: **102/102 PASS** (§4.1).

### 2.5 Chứng minh `netsrv` trả lời `shell` **trực tiếp** U-mode→U-mode (gỡ đường vòng `EP_REPLY`)

Đã **bỏ** `m.nb_send(T_KERN, ep_slot[3]=EP_REPLY, NET_REPLY)` trong `proc_rt::net_service`; thay
bằng bằng chứng **phía kernel** trên hàng của `shell`: `direct_pushed` = bộ đếm **luỹ kế** `pushed`
của `Endpoint EP_NET_REPLY` (đơn điệu ⇒ không phụ thuộc `shell` đã `NBRecv` hay chưa), và `fwd=0`
(không còn mã nào gửi hộ). Boot sản phẩm:

```text
# product net: served=1 denied=0 failed=0 err=0 rtt_ms=6 sent=1 received=1 label=0x291 caller=26 direct_pushed=0 fwd=0
# netsrv: ping 10.0.2.2 rtt_ms=6 err=0
# netsrv: reply nb err=0
# product net: served=1 denied=0 failed=0 err=0 rtt_ms=6 sent=1 received=1 label=0x291 caller=26 direct_pushed=1 fwd=0
# shell: ping 10.0.2.2 rtt_ms=6 err=0
```

Đọc theo thứ tự: kernel **phục vụ** `NetSvc` (`served=1 sent=1 received=1` — ICMP thật, `rtt_ms=6`);
`netsrv` nhận `rtt=6` rồi `NBSend` trả **0**; kernel thấy **đúng một** message do `netsrv` đẩy vào
hàng **của `shell`** (`direct_pushed=1`) và **không** gửi hộ (`fwd=0`); `shell` in `rtt_ms=6` —
giá trị **chỉ** có trong message của `netsrv` (kernel không còn chuyển tiếp). Trước `KT-342b`,
dòng `# shell: ping …` đến từ `EP_REPLY` của kernel (§2.1: `# shell: ping timeout` khi bỏ đường đó).

Đột biến (i) chứng minh phép kiểm có răng: **bỏ xếp hàng** ⇒ message mất ⇒ chính hai ca host (1)(2)
đỏ; và nếu ai bỏ `unlink_tcb_queues`-theo-`blocking`, ca (2) đỏ lại ngay.

---

## 3. Hai đột biến **ĐỎ**

| Đột biến | Sửa | Ca **đỏ** (log) |
|---|---|---|
| (i) `mut-drop-nb-no-waiter` | `endpoint_send`: **bỏ** message khi không có waiter | ca 1 + ca 2 |
| (ii) `mut-state-before-cap` | `nb_send`: kiểm **trạng thái TCB trước** kiểm cap | ca 4 |

```text
$ cargo test … -p vifast-core --features mut-drop-nb-no-waiter nb_queue
thread '…nb_send_khong_waiter_vao_hang' panicked:
  bất biến IPC (bảo toàn message) vỡ sau public op: CheckFailure { what: "message mat/them", a: 1, b: 0 }
failures: nb_send_khong_waiter_vao_hang, nb_send_song_sot_qua_exit_cua_sender
test result: FAILED. 2 passed; 2 failed

$ cargo test … -p vifast-core --features mut-state-before-cap nb_queue
  panicked: assertion `left == right` failed …   right: InvalidCapability
failures: loi_cap_dung_truoc_loi_trang_thai
test result: FAILED. 3 passed; 1 failed
```

Hai feature khai trong `vifast-core/Cargo.toml` (`[features]`), **không** dùng ở ảnh sản phẩm.

---

## 4. Số đo & cổng

### 4.1 Host test (T0)

| Lệnh | Kết quả |
|---|---|
| `cargo test -p vifast-core` | **102/102 PASS** (gồm 4 ca `nb_queue`) |
| `cargo test -p vifast-sim` (oracle) | **PASS** (mọi bộ, 0 fail) |
| `cargo test -p vifast-core --features mut-drop-nb-no-waiter nb_queue` | **ĐỎ đúng** (ca 1+2) |
| `cargo test -p vifast-core --features mut-state-before-cap nb_queue` | **ĐỎ đúng** (ca 4) |

### 4.2 Ràng buộc hiến định — **không đổi**

```text
# core: product addr=0x802470b8 bytes=184792 pin=196608 image_cap=262144 … footprint_ok=1
# budget: … data+bss=224472 budget=262144 …
```

`size_of::<Machine>()` = **184.792 B** (không đổi); `.data+.bss` sản phẩm `riscv64` =
**224.472 B** ≤ **262.144 B**.

### 4.3 Cổng (cây **sạch**, commit `d14e9201`)

**`./v2/bin/v2 check` — PASS**

```text
rules: PASS (/tmp/kt342b-s1/v2)
# step-time-total   100089 ms  TỔNG check
RC check=0
```

**`./v2/bin/v2 gate --dev` (T1) — PASS**

```text
# step-time-total   617590 ms  TỔNG gate --dev (T1)
v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')
RC gate=0
```

(T1, **không** phải full gate — đúng nhãn; full gate = `gate --fast` của Lead/T2.)

**`bash v2/test/boot/product-riscv64.sh` — `checks=0 fails=0` + `REV-GATE: ACCEPT`**

```text
ok 0 - budget: … file=552424 data+bss=224472 budget=262144 …
ok 0 - markers: 0 harness marker (4 mẫu) — D2-08
ok 0 - boot: shell U-mode lên (cmds=…/ping/run)
ok 0 - net: netsrv U-mode chạy
ok 0 - net: shell nhận reply của netsrv
ok 0 - net: kernel ping thật (served=1 sent=1 received=1)
ok 0 - net: reply trực tiếp U-mode→U-mode (direct_pushed=1, fwd=0)
ok 0 - net: netsrv tự báo NBSend trả 0
ok 0 - net: tổng kết replies=1
ok 0 - exec: bảng ảnh + exec nguyên tử / ảnh đích chạy thật (RUN OK)
ok 0 - halt: shell_exit=0 (thoát sạch) / execs=1
ok 0 - log: không PANIC / không 'not ok' / đúng một 'ok 1 - k3 boot'
# product-riscv64: checks=0 fails=0 image=…/vifast-kbin-riscv64 log=…/kt342-product.50640/kt342-product.log
# product-riscv64: rev-gate ACCEPT (D2-125.3)
RC product=0
```

Ba ca âm của runner (`feature-graph` / `no-netsrv` / `no-net`) chạy **cùng lượt** và đều đỏ
**đúng lý do** (14 + 3 + 2 phép kiểm riêng) — không đổi so với `KT-342`.

**`bash v2/test/boot/ktest.sh 1 matrix` — 27/27 PASS**

```text
# KTEST: 27/27 boot sạch (smp=matrix, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec ktest-ipc ktest-netsrv)
# ktest.sh: PASS
RC matrix=0
```

TAP từng ảnh **không đổi** so với `ktest-riscv.md` §6 (`ktest 71`, `ktest-guard 3`, `ktest-user 37`,
`ktest-proc 10`, `ktest-vblk 6`, `ktest-con 5`, `ktest-exec 6`, `ktest-ipc 11`, `ktest-netsrv 3`) —
đọc từ `v2/test/boot/expected.txt`, không nới.

### 4.4 Dòng bằng chứng cuối (log sản phẩm, cùng boot với cổng)

```text
# core: product addr=0x802470b8 bytes=184792 pin=196608 image_cap=262144 … footprint_ok=1
# product net: served=1 denied=0 failed=0 err=0 rtt_ms=6 sent=1 received=1 label=0x291 caller=26 direct_pushed=0 fwd=0
# netsrv: ping 10.0.2.2 rtt_ms=6 err=0
# netsrv: reply nb err=0
# product net: served=1 denied=0 failed=0 err=0 rtt_ms=6 sent=1 received=1 label=0x291 caller=26 direct_pushed=1 fwd=0
# shell: ping 10.0.2.2 rtt_ms=6 err=0
# product halt: shell_exit=0 fault_seen=1 events=4 … proc_up=1 fs_replies=28 … ring_ok=1
```

---

## 5. Cam kết phạm vi

Không sửa: `spec/**`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/**`, `.github/**`,
`docs/QUYET-DINH-sau-review.md`, `docs/v2/*`, `docs/v1/**`, 19 tệp `[[legacy]]` của
`numbers-policy.toml`, `main`. Không push. Không nới ngưỡng nào (R1 1.500, sản phẩm 262.144,
`Machine` 184.792, matrix 27/27, TAP từng ảnh không đổi). Không `#[ignore]`, không retry để lấy
xanh: lần chạy `ktest.sh` giữa chừng có **1 lần đỏ do tranh chấp logdir mặc định** với tiến trình
khác trên máy (`/var/folders/.../k3-ktest` bị ghi đè trong lúc chạy — log đọc lại thấy 37/37 trong
khi runner báo 13/37); sau đó mọi phép đo dùng `KT54_LOGDIR` **riêng** và đỏ thật `not ok 9` được
điều tra tới gốc (§2.3) rồi sửa bằng **thu hẹp luật**, không bằng nới kỳ vọng.

## 6. Nợ / đề xuất

1. **`KT-342c` (đề xuất, 2–3 h)**: mở `keep_committed` cho **mọi** đường chuyển trạng thái
   (§2.3 "Nợ còn lại") — hiện `NBSend` vẫn có thể mất nếu người gửi `Yield`/block/suspend trước khi
   bên nhận `Recv`. Cần sửa bất biến §4.4 + drain/khẳng định lại 9 ảnh test.
2. **Agent 0**: ghi ngữ nghĩa "`NBSend` trả `Ok` ⇒ message thuộc bên nhận, không bị rút khi người
   gửi chết" thành mục `D2-*` (tôi không sửa `DECISIONS.md`).
3. `user.rs` còn **704** dòng (< 1.100 ✔); ba mô-đun mới đều < 320 dòng — không cần tách thêm.

