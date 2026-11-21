# KT-281 — riscv64 **net server U-mode** (N1 phần 3)

Trạng thái: **đang làm** (nhánh `task/kt281-netsrv`, base `main` @ `20208255`), 2026-11-21.
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

_(điền sau khi chạy)_
