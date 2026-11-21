# KT-306 — riscv64 **`NetPing` thành syscall thật**

Trạng thái: **DONE** (nhánh `task/kt306-netping`, base `main` @ `58a1f867`), 2026-11-21.
Sub-agent `netping`. Quyết định: **D2-219** (vòng 18), **D2-04** (`spec/` cấm sửa), **D2-05**
(test đỏ trước), **D2-83** (core không đọc/ghi bộ nhớ user), **D2-109/D2-111** (tách ảnh khi thêm
đường mới), **D2-198** (uỷ quyền commit trên nhánh task).

**Không** tuyên bố K3/N1 đạt lại: KT-306 chỉ **trả một nợ đã công bố** trong `STATE.md`
("`NetPing` nên thành syscall thật") và **không** mở lại bất kỳ mục nào của hợp đồng K3 (đã đóng
theo D2-205).

## 1. Khảo sát trước (bắt buộc) — `NetPing` hiện ở đâu, ai gọi?

Đọc: `spec/abi.toml` (`[syscall]`/`[syscall_reserved]`), `v2/lib/vifast-abi/src/generated.rs`,
`v2/kernel/vifast-core/src/{syscall,invoke,ipc,core_ipc,tcb}.rs`,
`v2/kernel/vifast-plat-riscv64/src/{user,user_run,virtio_net,init_pages,obj}.rs`,
`v2/kernel/vifast-kbin-riscv64/src/{ktest_netsrv,ktest_net,proc_rt,proc_fs}.rs`,
`v2/kernel/vifast-kbin-riscv64/init/{netsrv_user,netcli_user}.rs`,
`v2/test/boot/{ktest.sh,expected.txt,net.sh,selftest.sh}`, `v2/agent/JOURNAL/{kt281-netsrv,kt282-mrbuf}.md`.

| Câu hỏi | Trả lời | Bằng chứng |
|---|---|---|
| `NetPing` hiện ở đâu? | **Hai chỗ**: (a) U-mode `netsrv` gửi `Call(EP_KERN, {label=0x291,length=1}, MR0=ip)` — **đã là syscall 0** từ KT-281; (b) **kernel xử lý nó bằng mã rig**: `ktest_netsrv.rs` gọi `m.recv(...)` → `virtio_net::ping(...)` → `m.reply_msg(...)` **trực tiếp trong `rust_main`** của ảnh test. Đây đúng là "hàm gọi trực tiếp trong rig" mà D2-219 nói. | `ktest_netsrv.rs` (bản KT-281) dòng "kernel: dịch vụ mạng"; `netsrv_user.rs:45` |
| Ai gọi `virtio_net::ping`? | **Rig** (`ktest_netsrv.rs`) và `ktest_net.rs` (ảnh `ktest-net`, toàn bộ stack trong kernel). Không có **dịch vụ kernel** nào sở hữu nó. | `grep -n "virtio_net::ping" v2/kernel/` |
| Có syscall nào **đã có** phù hợp không? | **CÓ** — syscall **0 `Call`** trên cap `Endpoint`. `spec/abi.toml` cấp đúng **16** số `0..15` (`SYSCALL_COUNT = 16`), dải `16..31` là `[syscall_reserved]`; `spec/` **cấm sửa** (D2-04) ⇒ **không** thêm số 16. `Call` có đúng ngữ nghĩa cần: chặn → kernel kiểm/xử lý → trả `(error, value)` qua `a0/a1`. | `spec/abi.toml:143..164`, `generated.rs:10`, `spec/abi.toml:144` |
| Vậy có cần syscall mới không? | **KHÔNG.** Đường đúng là `Call` + label `NetPing` trên `Endpoint` của **dịch vụ kernel** — cùng khuôn dịch vụ FS (`proc_fs::FsSvc` + `proc_rt::service`). | `proc_rt.rs:213..310` |
| Thiếu gì để gọi là "syscall thật"? | (1) **Chỗ cài đặt**: `virtio_net::ping` nằm trong **rig** (mã harness), không phải dịch vụ kernel ⇒ chương trình U-mode không có "syscall" nào để gọi, chỉ có một nhánh rig tình cờ phục vụ nó. (2) **Không kiểm quyền**: rig chỉ so `label == 0x291` — không kiểm **ai** gọi, không có chính sách IP, không có mã lỗi riêng cho từ chối. | `ktest_netsrv.rs` (bản KT-281) |
| Kernel có làm **server IPC** được không? | **CÓ** — `Machine::recv(tcb, ep, Some(reply))` + `reply_msg`; `proc_rt::service()` là dịch vụ kernel thật (FS, `ps`, fault). | `core_ipc.rs:554,654`, `proc_rt.rs:143..310` |
| U-mode `Call` **chặn** + **resume** được chưa? | **CÓ** (KT-280/KT-281): `sepc += 4` ở nhánh `Switched` + `apply_ipc_return` nạp `a0..a6` từ `tcb_ipc_area`. | `user.rs` nhánh `Switched`, `user_run.rs:32..46` |

### 1.1 Kết luận khảo sát

1. **Không** cần syscall mới (và **không được** thêm: `spec/` cấm sửa, dải `16..31` reserved).
2. Nợ thật của `NetPing` **không** phải "thiếu số syscall" — nó là **thiếu dịch vụ kernel**: phép
   ping được cài trong **rig** nên (a) không có mã sản phẩm nào dùng lại được, (b) không có **kiểm
   quyền** trước khi chạm thiết bị.
3. Đường đúng: **dịch vụ kernel `NetPing`** trên `Endpoint` (`net_svc::NetSvc`), U-mode gọi bằng
   syscall **0 `Call`**; dịch vụ **kiểm quyền** rồi mới gọi driver, trả `(error, value)`.

### 1.2 Vì sao **không** làm dịch vụ chạy **ngay trong** trap handler

Đã cân nhắc và **loại** (ghi lại để người sau không đi lại): chạy `virtio_net::ping` **bên trong**
`on_u_exception` (để một `ecall` trả kết quả ngay) đòi `satp` = root user lúc đó, mà
`init_pages.rs:102` ghi rõ **"nửa cao (kernel high alias + physmap) KHÔNG map trong VSpace user"**
⇒ driver chạm DMA/physmap sẽ page-fault. Đường an toàn là **đổi `satp`** trong trap handler
(`obj::restore_kernel_root`/`switch_root`) — thay đổi rủi ro cao, không cần cho tiêu chí của
D2-219, nên **không** làm trong task 3 h. Ghi thành **nợ** (§7).

## 2. Thiết kế

### 2.1 Dịch vụ kernel `net_svc::NetSvc` (mới)

`v2/kernel/vifast-kbin-riscv64/src/net_svc.rs` — cùng khuôn `proc_fs::FsSvc`:

```text
server U-mode (netsrv)                          kernel (dịch vụ NetPing)
  Call(EP_KERN, {label=0x291,length=1}, MR0=ip) ──►  recv(EP_KERN, REPLY)
    [BlockedOnSend{call}]                            KIỂM QUYỀN (caller/label/length/IP)
                                                     virtio_net::ping(...)  ← chỗ DUY NHẤT chạm thiết bị
  resume: a0=0, a1=1, a2=MessageInfo, a3=rtt ◄────  reply_msg(REPLY, {label=0x291}, MR0=rtt)
```

**Kiểm quyền** (fail-closed — từ chối thì **không** gửi gói nào):

| Điều kiện | Mã trả |
|---|---|
| `Reply.caller` **không phải** TCB net server đã cấp cap | `InsufficientRights` (3) |
| `label != 0x291` hoặc `length != 1` | `InvalidArgument` (5) |
| `MR0` ngoài **chính sách IP** (gateway hoặc cùng `/24` với địa chỉ DHCP) | `InvalidArgument` (5) |
| driver lỗi (`Timeout`/`Io`/…) | `Timeout` (16) — giữ nguyên ngữ nghĩa KT-281 |

Chính sách IP là phần **quyền** thật: không có nó thì bất kỳ chương trình U-mode nào giữ cap cũng
biến kernel thành máy phát ICMP tuỳ ý (khuếch đại/dò mạng).

### 2.2 Ảnh `ktest-netsrv`: 2 → **3 TAP**, 4 → **5 lượt** tất định

| Lượt | Tác vụ | Việc | `Stop` |
|---|---|---|---|
| 1 | client | `Call(EP_SRV, Ping(ip))` | `Blocked` |
| 2 | server | `Recv` → `Call(EP_KERN, NetPing)` | `Blocked` |
| — | **dịch vụ #1** | `NetSvc::service` → `Served` (ping thật) | — |
| 3 | server | resume: in `# netping:` → `Call(EP_KERN, label SAI)` | `Blocked` |
| — | **dịch vụ #2** | `NetSvc::service` → `Denied` (**không** gửi gói) | — |
| 4 | server | resume: in `# netping: neg` → `Send(Reply, rtt)` → `Exit` | `Exit` |
| 5 | client | resume: kiểm reply + 2 ca âm → `Exit` | `Exit` |

TAP mới: **`ok 3 - netping: via ipc`** — đòi **đồng thời**: `outcome1 == Served`,
`outcome2 == Denied`, `served=1`, `denied=1`, `failed=0`, `label` request đúng `0x291` rồi `0x999`,
`caller` = TCB server, `err = 5`, **`sent = 1`** (request label sai **không** gửi gói nào), reply
trong vùng nhận IPC của server đúng `label`/`MR0`, và hai dòng do **chính** chương trình U-mode in.

### 2.3 Đo

```text
# netping kern: n=1 outcome=Served served=1 denied=0 failed=0 label=0x291 len=1 mr=0xa000202 caller=4 err=0 rtt_ms=0 sent=1 received=1 reply_label=0x291 reply_mr=0 server=Ready
# netsrv: ping 10.0.2.2 ok rtt=0
# netping: label=0x291 reply=0 rtt_ms=0
# netping kern: n=2 outcome=Denied served=1 denied=1 failed=0 label=0x999 len=1 mr=0xa000202 caller=4 err=5 rtt_ms=0 sent=1 received=1 reply_label=0x999 reply_mr=5 server=Ready stop=Blocked
# netping: neg label err=5 ok=1
ok 1 - netsrv: server ran
ok 2 - netsrv: client got reply
ok 3 - netping: via ipc
```

`# netping: label=0x… reply=… rtt_ms=…` do **chính server U-mode** in: `label` lấy từ
`MessageInfo` của **reply** (kernel echo label request), `reply` = `a0`, `rtt_ms` = `MR0` — cả ba
là **số đo**, không phải hằng của bài test.
