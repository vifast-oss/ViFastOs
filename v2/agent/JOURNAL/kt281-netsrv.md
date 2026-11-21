# KT-281 — riscv64 **net server U-mode** (N1 phần 3)

Trạng thái: **DONE** (nhánh `task/kt281-netsrv`, base `main` @ `20208255`), 2026-11-21.
Sub-agent `netsrv`. Quyết định: **D2-196** (mở D1), **D2-210** (vòng 10), **D2-198** (uỷ quyền
commit trên nhánh task), **D2-05** (test đỏ trước), **D2-109/D2-111** (tách ảnh khi thêm đường mới),
**D2-211** — *không tìm thấy mục này trong `v2/agent/DECISIONS.md`* (tệp dừng ở **D2-210**, 397
dòng); xem §1.4.

**Không** tuyên bố K3/N1 đạt: KT-281 chỉ thêm **một đường mới** (server U-mode điều phối dịch vụ
mạng của kernel) và **không** mở lại bất kỳ mục nào của hợp đồng K3 (đã đóng theo D2-205).

## 1. Khảo sát trước (bắt buộc) — riscv64 đã có gì cho mạng?

Đọc: `v2/kernel/vifast-core/src/{syscall,invoke,core_ipc,machine}.rs`,
`v2/kernel/vifast-plat-riscv64/src/{user,user_run,virtio_net,pci}.rs`,
`v2/kernel/vifast-kbin-riscv64/src/{ktest_ipc,ktest_net,proc_rt,proc_fs}.rs`,
`v2/kernel/vifast-kbin-riscv64/init/{shell_user,ipc_client_user,ipc_server_user}.rs`,
`v2/kernel/vifast-plat-user/lib.rs`, `v2/lib/vifast-net/src/**`, `v2/docs/d1-net.md`,
`v2/test/boot/{ktest.sh,net.sh,expected.txt}`, `spec/abi.toml`.

| Câu hỏi | Trả lời | Bằng chứng |
|---|---|---|
| riscv64 có **syscall mạng** nào chưa? | **CHƯA.** `spec/abi.toml` `[syscall]` cấp đúng **16** số `0..15`; dải `16..31` là `[syscall_reserved]`. `SYSCALL_COUNT = 16` (`v2/lib/vifast-abi/src/generated.rs:10`). Không có `NetPing`/`NetInfo`/`Socket`. | `spec/abi.toml:143..164`, `generated.rs:10` |
| `shell_user.rs` có `ping`/`ifconfig` không? | **KHÔNG.** `grep -c 'ping\|ifconfig'` = 2 nhưng **cả hai** là `Sleeping`/`Stopping` (dòng 239/240) — không có lệnh mạng nào. Shell sản phẩm chỉ có `help/echo/ps/wait/kill/exit` (+ `ls/cat/write` khi `product-fs`). | `shell_user.rs:239..240`, `proc_demo.rs` `# shell: help cmds=6` |
| Có **tái dùng** được syscall hiện có cho `NetPing`? | **Không trực tiếp**: 16/16 số đã cấp nghĩa, `spec/` **cấm sửa** (luật task), nên **không** thêm được số 16. **Nhưng** có đường tái dùng **đúng nghĩa ABI**: syscall **0 `Call`** trên cap **`Endpoint`** = IPC chuẩn — kernel đóng vai **server mạng** trên một Endpoint, đúng khuôn `proc_fs.rs`/`proc_rt.rs` đã có cho dịch vụ FS (`shell` ↔ kernel FS). | `spec/abi.toml:144`, `proc_rt.rs:213..310` |
| Kernel có làm **server IPC** được không? | **CÓ** — `proc_rt.rs::service()` `nb_recv`/`nb_send` trên endpoint của TCB `kern`; `proc_fs.rs::FsSvc` là dịch vụ FS thật. `Machine::recv(tcb, ep, Some(reply_slot))` + `reply_msg` cho đường **chặn** (`Call`). | `proc_rt.rs:143..310`, `core_ipc.rs:554..700` |
| U-mode `Call` **chặn** + **resume** được chưa? | **CÓ** (KT-280): `sepc += 4` ở nhánh `Switched`, `apply_ipc_return` nạp `a0..a6` từ `tcb_ipc_area`, scheduler giữ tác vụ chặn sống. Ảnh `ktest-ipc` chạy 2 tác vụ U-mode, TAP 2/2. | `user.rs` (nhánh `Switched`), `user_run.rs:32..46`, `ktest_ipc.rs` |
| Driver mạng có API gì? | `virtio_net::{init(&PciReport), dhcp_acquire, arp_resolve, ping, mac, ready, stats}`; `ping(our_ip, our_mac, target_ip, target_mac, count, timeout_ticks) -> Result<PingStats, NetError>`; `PingStats{rtt_min_ms, rtt_max_ms, sent, received}`. | `virtio_net.rs:315,913,945,975` |
| Codec mạng ở đâu? | `v2/lib/vifast-net` — `no_std`, `deny(unsafe_code)`, **không** dependency; `eth/arp/ipv4/icmp/udp/dhcp/dns/tcp`. **Không** viết lại codec (luật task). | `v2/lib/vifast-net/src/**` |
| Ảnh test mạng hiện có? | `ktest-net` (11 TAP, runner `net.sh`) — **toàn bộ** stack chạy **trong kernel**, không có U-mode. `d1-net.md` §8 mục 2: *"Net server U-mode — chưa có"*. | `ktest_net.rs`, `net.sh`, `d1-net.md:120` |

### 1.1 Kết luận khảo sát

1. **Không** có syscall mạng, **không** có lệnh mạng trong shell, **không** có net server U-mode.
   Đây là lỗ hổng thật của N1 phần 3.
2. **Không** được thêm số syscall (spec cấm sửa) ⇒ `NetPing` **phải** đi qua **syscall 0 `Call`
   trên Endpoint** — đúng câu chữ "tái dùng syscall hiện có" của đề bài, và đúng khuôn dịch vụ
   kernel đã có (`proc_fs`).
3. Driver **ở kernel** (D2-211 tinh thần: "driver ở kernel, server chỉ điều phối"); server U-mode
   **không** map MMIO — nó chỉ giữ cap `Endpoint`, không giữ cap `Frame`/BAR.

### 1.2 Vì sao **không** thêm syscall 16 (và vì sao điều đó không hạ phạm vi)

`spec/abi.toml` là **nguồn sự thật của ABI** và luật vòng này **cấm sửa `spec/`**. Một syscall mới
cần: số trong `[syscall]`, mã sinh `vifast-abi`, `SYSCALL_COUNT`, nhánh `dispatch`, hằng shim
U-mode — tức **5 chỗ** phải đổi đồng thời, và `check_abi.py`/`gen-check` sẽ đỏ nếu lệch. Đường
`Call` trên Endpoint cho **cùng ngữ nghĩa** (server U-mode gọi kernel, kernel chặn–trả lời, server
resume với `(error, value)`) mà **không** đổi một byte ABI nào.

### 1.3 Phạm vi giữ nguyên (không hạ)

Đề bài cho phép hạ xuống "`NetInfo` trả MAC" nếu syscall mạng khó. **Không cần hạ**: đường
`Call`→kernel endpoint chạy được **ping thật** (`virtio_net::ping` → ICMP echo → slirp trả lời),
nên KT-281 làm **đúng phạm vi gốc**: `# netsrv: ping 10.0.2.2 ok rtt=<ms>`.

### 1.4 D2-211

`v2/agent/DECISIONS.md` (397 dòng) dừng ở **D2-210**; `grep -rn "D2-211"` toàn repo = **0 hit**.
Không sửa `DECISIONS.md` (luật task) ⇒ ghi nhận ở đây và bám **D2-210** + đề bài của lead.

## 2. Thiết kế

Ảnh **mới** `vifast-kbin-ktest-netsrv` (feature `netsrv = ["ktest","user","vifast-plat-riscv64/net"]`,
bin riêng) theo D2-109/D2-111 (thêm đường mới ⇒ tách ảnh, không nới trần, `.text`/TAP của 8 ảnh cũ
**không đổi**).

```text
client U-mode (netcli)          server U-mode (netsrv)            kernel (driver)
  Call(EP_SRV, Ping(ip))  ──►   Recv(EP_SRV) → ip
    [chặn BlockedOnSend]          Call(EP_KERN, NetPing(ip)) ──►  recv(EP_KERN, reply)
                                    [chặn BlockedOnSend]           virtio_net::ping(...)
                                                                   reply_msg(rtt)
  ◄── Send(Reply, rtt)  ◄────   resume: a0=0, a1=rtt_ms
  resume: a0=0, a1=rtt_ms
  in `# netsrv: client got reply=<v>`   in `# netsrv: ping 10.0.2.2 ok rtt=<ms>`
```

- **client** `netcli_user.rs`: `Call(EP_SRV, {label=0x290,length=1}, MR0=ip)` ⇒ chặn; resume ⇒ in
  `# netsrv: client got reply=<v>`; ca âm `Call` trên ô rỗng ⇒ `err=1`; `Exit(0)`.
- **server** `netsrv_user.rs`: `Recv(EP_SRV, REPLY_SLOT)` ⇒ `ip`; `Call(EP_KERN,
  {label=0x291,length=1}, MR0=ip)` ⇒ chặn; resume ⇒ `a0=error, a1=rtt`; in
  `# netsrv: ping 10.0.2.2 ok rtt=<ms>`; `Send(REPLY_SLOT, {label=0x292,length=1}, MR0=rtt)` ⇒
  client resume; `Exit(0)`.
- **kernel** `ktest_netsrv.rs`: PCI + `virtio_net::init` + DHCP + ARP gateway; rig 3 TCB
  (client/server/kern) + 2 Endpoint (`EP_SRV`, `EP_KERN`) + 2 Reply + 3 cspace + 3 SC; scheduler
  **4 lượt tất định**; giữa lượt 2 và 3 kernel `recv(EP_KERN, reply)` → `virtio_net::ping` →
  `reply_msg`. TAP 1 = `netsrv: server ran`, TAP 2 = `netsrv: client got reply`.
- **Ca âm**: không `-netdev`/`-device` ⇒ `# netsrv: no device` + `not ok 1` + `halt(1)`.

**Vì sao driver vẫn ở kernel**: server U-mode chỉ giữ cap `Endpoint` (không `Frame`, không BAR,
không `MapIo`) ⇒ nó **không thể** chạm thiết bị kể cả muốn; mọi truy cập MMIO nằm trong
`vifast-plat-riscv64::virtio_net` (crate arch, `unsafe` trong danh sách D2-17).

## 3. Kết quả

| Việc | Kết quả |
|---|---|
| Ảnh mới `vifast-kbin-ktest-netsrv` (feature `netsrv = ["ktest","user","vifast-plat-riscv64/net"]`) | **DONE** — 2 TAP, `.data+.bss` = **217.952 B** / 262.144 (headroom 44.192 B) |
| Hai chương trình U-mode `init/netcli_user.rs` / `init/netsrv_user.rs` | **DONE** — 8.928 B / 8.904 B ELF |
| Kernel `src/ktest_netsrv.rs` — rig 3 TCB + 2 Endpoint + 2 Reply + 3 cspace + 3 SC, 4 lượt tất định, kernel `recv`+`reply_msg` giữa lượt 2 và 3 | **DONE** |
| Runner `ktest.sh`: ảnh thứ **chín** (`LABELS`/`image_path`/`manifest_netsrv`/`evidence_for` 15 mục/`panic_allow_re`/build/`qextra` `-netdev`) | **DONE** — matrix **9 ảnh × smp 1/2/4 = 27 boot** |
| Runner `net.sh`: ca âm `# netsrv: no device` + `--red-selftest` **+5 kiểm** (28 → **33**) | **DONE** |
| `./v2/bin/v2 check` | **XANH** (exit 0; 118,8 s) |
| Runner riscv64 `ktest.sh 1 matrix` | **XANH — 27/27 boot sạch** |
| Ba đột biến | **ĐỎ đúng cả ba** (§6) |

Commit trên nhánh `task/kt281-netsrv` (base `20208255`):

| Commit | Nội dung |
|---|---|
| **`10035055`** | `[KT-281] net server U-mode riscv64: client Call chặn → server điều phối gọi dịch vụ mạng của kernel → Reply(rtt) (TAP 2/2, ảnh ktest-netsrv thứ chín)` — 9 tệp, +1.384/−3 |

Tổng so với base: **9 tệp** (không tệp nào trong `v2/agent/{STATE,QUEUE,DECISIONS}.md`,
`v2/bin/v2`, `spec/`).

## 4. Log QEMU nguyên văn (smp=1, ảnh `ktest-netsrv`, commit `10035055`)

```text
TAP version 13
1..2
# netsrv image: hart=0 stvec=0x802000a0 satp=0x80000000000804e8 mode=8 tables=10 used=40960 const:target=10.0.2.2
# netsrv net: mac=52:54:00:12:34:56 dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2
# netsrv arp: gw=52:55:0a:00:02:02
# netsrv rig: untyped=0 core_untyped=1 cspace_c=14 cspace_s=15 cspace_k=16 tcb_c=3 tcb_s=4 tcb_k=5 ep_srv=6 ep_kern=7 reply_client=8 reply_kern=9
# netsrv call: blocked=1 state=BlockedOnSend call=1 ep_len=1 ep_mr=0xa000202 label=0x290 stop=Blocked
# netsrv server: recv=1 label=0x290 mr=0xa000202 client=BlockedOnReply caller=3 server=BlockedOnSend kern_blocked=1 kern_len=1 kern_mr=0xa000202 stop=Blocked
# netsrv kern: recv=1 label=0x291 mr=0xa000202 server=BlockedOnReply caller=4 ping=ok sent=1 received=1 rtt_ms=0 reply=ok
# netsrv: ping 10.0.2.2 ok rtt=0
# sys: nr=14 err=0 value=0 tcb=4 state=Inactive src=core
# u-mode exit: code=0 faultkind=Exit{0} tcb=4 state=Inactive
# netsrv reply: applied=1 server_exit=0 client=Ready reply_caller=0 server=Inactive stop=Exit
# netsrv: client got reply=0
# netsrv: neg empty=1 reply=18 ok=1
# sys: nr=14 err=0 value=0 tcb=3 state=Inactive src=core
# u-mode exit: code=0 faultkind=Exit{0} tcb=3 state=Inactive
# netsrv client: reply_label=0x292 reply_mr0=0 const:label_reply=0x292 const:rtt_ms=0
ok 1 - netsrv: server ran
ok 2 - netsrv: client got reply
# netsrv final: client_exit=0 server_exit=0 reply_clean=1 check_ipc=1 check_caps=1 capture_client=1 capture_server=1 client=Inactive server=Inactive
PANIC: KT-281 netsrv xong (2 TAP, 0 fail)
```

Đọc dòng theo dòng (mỗi dòng là một phép đo, không phải lời khai):

1. `# netsrv net:` / `# netsrv arp:` — MAC **đọc từ `virtio_net_config`** (`52:54:00:…`, OUI QEMU),
   DHCP thật của slirp (`ip=10.0.2.15 gw=10.0.2.2`), ARP gateway thật (`52:55:0a:00:02:02`). Đây là
   **driver trong kernel** làm, trước khi có tác vụ U-mode nào.
2. `# netsrv call:` — client U-mode `Call` trên `EP_SRV` **khi chưa có người nhận** ⇒ core đưa TCB
   client sang `BlockedOnSend{call:true}`, message `Ping(ip)` nằm trong **hàng Endpoint**
   (`ep_len=1`, `ep_mr=0xa000202` = `10.0.2.2`). `stop=Blocked` = nhánh `Switched` chạy thật.
3. `# netsrv server:` — server U-mode `Recv` lấy message: caller chuyển
   `BlockedOnSend{call}` → **`BlockedOnReply`** (`caller=3` = TCB client), message server nhận đúng
   `label=0x290 mr=0xa000202`. **Rồi** server `Call` **kernel** trên `EP_KERN` ⇒ chặn ở Send
   (`server=BlockedOnSend`, `kern_len=1`, `kern_mr=0xa000202`). Đây là **bằng chứng trực tiếp** của
   kiến trúc "server chỉ điều phối": server không có cap thiết bị nào, chỉ có hai cap `Endpoint`.
4. `# netsrv kern:` — kernel `recv` trên `EP_KERN` **kèm cap Reply** ⇒ server `BlockedOnReply`
   (`caller=4` = TCB server), message đúng `label=0x291 mr=0xa000202`; kernel chạy
   `virtio_net::ping` **thật** ⇒ `ping=ok sent=1 received=1 rtt_ms=0`, rồi `reply_msg` (`reply=ok`).
5. `# netsrv: ping 10.0.2.2 ok rtt=0` — **chính server U-mode** in (qua `DebugPutStr`), sau khi
   **được resume** từ `Call` chặn: `a0=0`, `a3` = RTT. IP in ra là **giá trị nhận được từ client**
   (`put_ipv4`), không phải hằng của kernel.
6. `# netsrv reply:` — server `Send` trên cap `Reply` ⇒ client `Ready`, `Reply.caller` **sạch** (0),
   server `Inactive` (Exit 0).
7. `# netsrv: client got reply=0` — **chính client U-mode** in sau khi **được resume**:
   `a0=0`, `a2` = `MessageInfo{label=0x292,length=1}`, `a3` = RTT.
8. `# netsrv: neg empty=1 reply=18 ok=1` — hai ca âm trong cùng boot: `Call` trên ô rỗng ⇒ **1**
   (`InvalidCapability`), `Call` trên cap `Reply` (không phải Endpoint) ⇒ **18** (`NotSupported`).
9. `# netsrv client:` — kernel đọc **độc lập** vùng nhận IPC của client: `reply_label=0x292
   reply_mr0=0` khớp `const:rtt_ms=0` (không tin lời chương trình).
10. `# netsrv final:` — `check_ipc`/`check_caps` xanh, `capture_client=1`/`capture_server=1`
    (output của **cả hai** chương trình U-mode được chụp đúng ô), hai TCB `Inactive`.

`rtt=0` là **số đo thật** dưới `-icount shift=6` (đồng hồ ảo) — cùng hiện tượng đã ghi ở
`docs/d1-net.md` §5 cho `ktest-net`; điều kiện của TAP là **có** reply (`received == sent == 1`),
không phải RTT khác 0.

## 5. TAP + matrix

```text
$ bash v2/test/boot/ktest.sh 1 matrix
# expected[ktest-netsrv]: plan=2 ok=2 ten-TAP=2
# image=ktest-netsrv path=…/vifast-kbin-ktest-netsrv file=424440 data+bss=217952 budget=262144 …
ok - ktest-netsrv smp=1 boot 1 (2 TAP, 1s) log …/ktest-netsrv-smp1-1.log
ok - ktest-netsrv smp=2 boot 1 (2 TAP, 1s) log …/ktest-netsrv-smp2-1.log
ok - ktest-netsrv smp=4 boot 1 (2 TAP, 0s) log …/ktest-netsrv-smp4-1.log
# KTEST: 27/27 boot sạch (smp=matrix, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec ktest-ipc ktest-netsrv)
# ktest.sh: PASS
```

`--check-log` (15 evidence bắt buộc):

```text
$ bash v2/test/boot/ktest.sh --check-log ktest-netsrv /tmp/kt281-final.log 1
ok - check-log ktest-netsrv smp=1 (plan=2 ok=2, 15 evidence) log /tmp/kt281-final.log
# check-log: PASS
```

**Ca âm thường trực** (`net.sh`, cùng ảnh, **không** `-netdev`/`-device`):

```text
# netsrv pci: err=NoDevice stage=nodevice
# netsrv: no device
not ok 1 - netsrv: server ran: khong tim thay thiet bi mang
# ktest halt
```

`net.sh --red-selftest` = **33 kiểm, 0 lỗi** (trước KT-281: 28). Năm kiểm mới:

```text
ok 29 - selftest: netsrv ca âm 'không thiết bị' đỏ đúng cách được chấp nhận
ok 30 - selftest: netsrv ca âm PASS giả (có 'ok 1') bị từ chối
ok 31 - selftest: netsrv ca âm thiếu dòng '# netsrv: no device' bị từ chối
ok 32 - selftest: netsrv ca âm có dòng PANIC 'xong' (PASS giả) bị từ chối
ok 33 - selftest: netsrv ca âm có dòng '# netsrv: ping' (bằng chứng giả) bị từ chối
# net.sh --red-selftest: 33 kiểm, 0 lỗi
```

## 6. Đột biến (đỏ có kiểm soát) — cả ba ĐỎ đúng

Script: `/tmp/kt281-mutations.sh` (sửa **một** chỗ → build → boot thật qua `qemu_guard.sh` →
`ktest.sh --check-log` phải đỏ → `git checkout --` trả tệp). Cây sạch sau cả ba.

| # | Đột biến | Kết quả log | Runner |
|---|---|---|---|
| M1 | server **không** `Reply` (bỏ `Send` trên cap Reply) | `# netsrv: ping 10.0.2.2 ok rtt=0` + `ok 1`; `not ok 2 … applied_c=0 client_exit=-1 said_reply=0 reply_label=0xffffffff`; `PANIC: KT-281 netsrv CO LOI` — **không treo**, kernel phát hiện client chưa nhận message và dừng có chẩn đoán | **ĐỎ**: `thiếu TAP #2`, `thiếu evidence#9..#13` |
| M2 | **syscall trả lỗi**: server `Call` trên cap `Reply` (không phải Endpoint) ⇒ `err=18` | `# netsrv: ping err=18`; `# netsrv: client got reply=18`; `not ok 1 … ping_ok=0 sent=0 received=0 said_ping=0`; `not ok 2 … reply_mr0=18 rtt_ms=0` | **ĐỎ**: `thiếu TAP #1/#2`, `thiếu evidence#6/#7/#8/#13` |
| M3 | client `Call` **sai endpoint** (ô 5 rỗng) ⇒ `err=1` | `# netsrv: client call err=1`; `not ok 1 … blocked_send=0 ep_len=0`; `not ok 2 … applied_s=0 applied_c=0` | **ĐỎ**: `thiếu TAP #1/#2`, `thiếu evidence#5..#8` |

M1 là ca "server không Reply" theo yêu cầu: thay vì treo 20 s, ảnh **phát hiện** client chưa có
message trong vùng nhận IPC (`apply_ipc_return` trả `false`) và in `not ok 2` +
`PANIC … CO LOI` — đỏ có kiểm soát, có chẩn đoán, không treo máy.

## 7. Còn gì chưa xong / rủi ro

1. **Không** làm (đúng phạm vi task): driver trong U-mode, nhiều client, TCP/DNS qua server,
   preemption giữa các tác vụ (scheduler của ảnh là **tuần tự tất định** 4 lượt, không phải
   scheduler ưu tiên đầy đủ).
2. **Không** tuyên bố K3/N1 đạt — K3 đã đóng theo D2-205; KT-281 chỉ thêm **một đường mới**
   (server U-mode điều phối dịch vụ mạng của kernel) và **không** mở lại mục nào của hợp đồng K3.
3. **Nợ (thiết kế)**: `NetPing` đi qua **syscall 0 `Call` trên Endpoint** chứ không phải một số
   syscall riêng, vì `spec/abi.toml` cấp đúng 16 số `0..15` và luật vòng này **cấm sửa `spec/`**.
   Ngữ nghĩa giống hệt (chặn → kernel xử lý → `(error, value)`), nhưng nếu sau này A0 mở
   `[syscall]` cho một số `NetPing` thật thì đường này nên được thay, **không** để song song hai
   đường. Ghi ở §1.2.
4. **Nợ**: `Stop::Blocked(TcbState::Inactive)` vẫn mang `Inactive` (bảng mã `decode` của
   `user_run.rs` không đổi) — ảnh KT-281 đọc trạng thái **thật** từ core (`Machine::tcb_state`)
   thay vì tin `Stop`; sửa `decode` là việc của task khác (chạm mọi ảnh).
5. **Rủi ro thấp**: ảnh `ktest-netsrv` cần `-netdev`/`-device` trong dòng QEMU (như `ktest-net`);
   `ktest.sh` thêm `qextra` theo nhãn. Nếu ai bỏ `qextra` đó, ảnh in `# netsrv: no device` +
   `not ok 1` ⇒ matrix **đỏ** ngay (không PASS giả).
6. **Chưa chạy T2 `gate --fast`** (task yêu cầu `v2 check` + runner riscv64; T2 là việc của lead
   trên ứng viên tích hợp).

## 8. Xác nhận luật

- **Không** push, **không** merge, **không** chạm `main` (nhánh `task/kt281-netsrv`, commit
  `10035055`).
- **Không** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `v2/bin/v2`, **không** sửa
  `spec/`.
- **Không** nới ngưỡng, **không** `#[ignore]`/retry, **không** `cfg(target_arch)` trong
  `vifast-core`/`vifast-plat-common` (không chạm hai crate đó).
- **Không** viết lại codec mạng — dùng `vifast_net` (`Ipv4Addr`/`MacAddr`) và driver
  `virtio_net` sẵn có.
- Stage theo **đường dẫn cụ thể** (9 tệp), **không** `git add -A`.
- Cây sạch sau commit + sau ba đột biến (`git status --short` rỗng).
