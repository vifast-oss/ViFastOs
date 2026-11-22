# KT-339 — `vifast-plat-user` đa kiến trúc + K3.5 trên aarch64 (`procsrv` + `shell` **cùng nguồn**)

- **Task**: `KT-339` (D2-233 mục 3, D2-242, D2-245), 3 h, nhánh `task/kt339-user`, base `main` @
  `4b78299b`, worktree `/tmp/kt339-s1`.
- **Phạm vi**: `v2/kernel/vifast-plat-user` (shim + `programs/`), `v2/kernel/vifast-plat-aarch64`
  (`userload`, `k5cap`, `syscall_env`, `bootmem`), `v2/kernel/vifast-kbin-aarch64` (`build.rs`,
  `k5user`, `coretest`, `umode`, `highentry`, `main`, `Cargo.toml`),
  `v2/kernel/vifast-kbin-riscv64/build.rs`, `v2/test/boot/{run,soak}-aarch64.sh`. **Không** chạm
  `spec/`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/v2`, `main`, 19 tệp `[[legacy]]`.
- **Kết quả một dòng**: TAP aarch64 **41 → 44** (42 `procsrv` 2 yêu cầu, 43 `shell` prompt +
  `help`/`echo` + `Exit(0)`, 44 `echo` đọc/ghi IPC buffer 8 MR); hai đột biến bắt buộc **ĐỎ**;
  ảnh **sản phẩm** vẫn **không** chứa K5 (lý do ngân sách §4).

---

## 1. Khảo sát trước khi sửa

### 1.1 `vifast-plat-user` trước KT-339

- **Một tệp** `lib.rs` 624 dòng, biên dịch bằng `$RUSTC` (không phải crate workspace) bởi
  `vifast-kbin-riscv64/build.rs`; **asm `ecall` nằm thẳng trong** `syscall7`/`syscall2`/`exit`.
- API công khai: `syscall7`/`syscall2`/`call`/`send`/`nb_send`/`recv`/`nb_recv`/`put`/`exit`,
  hằng `SYS_*`, `IPC_BUFFER_VA = 0x4002_0000`, header IPC buffer (D2-98) + `mrs()`/`set_mr()`.
- **Không** có khái niệm backend: mọi chương trình U-mode (kể cả `procsrv`/`shell`) buộc phải
  chạy trên riscv64 vì shim chỉ có `ecall`.

### 1.2 Khác biệt stub syscall hai arch (nguồn: `spec/abi.toml`)

| | riscv64 (`[calling.riscv64]`) | aarch64 (`[calling.aarch64]`) |
|---|---|---|
| lệnh vào kernel | `ecall` | `svc #0` |
| số hiệu syscall | `a7` | `x8` |
| tham số chung | `a0..a5` | `x0..x5` |
| `MessageInfo` khi **gửi** IPC | **`a1`** (cap `a0`, MR0..3 `a2..a5`) | **`x2`** (cap `x0`, MR0..3 `x3..x6`) |
| trả về | `a0..a6` = `(error, value, info, MR0..MR3)` | `x0..x6` = cùng thứ tự |
| `ELR`/`sepc` sau trap | `sepc += 4` ở arch | `ELR_EL1` **đã** là instruction kế (DDI 0487 §D17.2.37) |
| VA IPC buffer | `0x4002_0000` (D2-98) | `0x7000_2000` (trang ảnh aarch64 bind cho TCB) |

Hệ quả: **không thể** chỉ `cfg` vài thanh ghi trong thân `procsrv`/`shell`; phải đưa toàn bộ
khác biệt vào **một** backend của shim.

### 1.3 `procsrv`/`shell` đang giả định gì

- Chỉ dùng façade `vifast_user_abi::{put, syscall7, exit}` — **không** asm, **không** địa chỉ vật
  lý, **không** `cfg(target_arch)`; hằng giao thức thuần số (`EP_*`, label, `INFO_LEN_SHIFT`).
- `procsrv` **định nghĩa `fn send` riêng** gọi `syscall7(SYS_NB_SEND, ep, info, mr[0..3])` (chữ ký
  "một cap + `MessageInfo`"); `shell` cũng vậy cho `send`/`recv`. Vì vậy façade `syscall7` **phải**
  dịch đúng ABI gửi của từng arch (xem §2.2) — đây là chỗ TAP 42/43 đã bắt lỗi thật trong lượt
  chạy 2026-11-23.
- `shell` đọc byte vào từ **ring KT-69** ở VA cố định `0x4300_0000` (kernel map + bơm), nên nguồn
  vào không phải "message kernel cắm sẵn" — cùng đường cho cả hai arch.
- `procsrv` vòng `NBRecv`/`Yield`; với "kernel là scheduler tuần tự" (KT-338b §8.2), mỗi `Yield`
  là một điểm kernel có thể **dịch vụ** giữa hai `svc` — đủ phục vụ 2 yêu cầu mà không cần
  scheduler preemptive.

### 1.4 Ảnh aarch64 trước KT-339

- `coretest` (TAP 36..41) dựng `Machine`, CSpace 16 ô, 1 Endpoint, 1 Frame IPC buffer ở
  `0x7000_2000`; EL0 chỉ có **stub asm 1 trang** (`.el0`) — chưa có loader ELF thật.
- Ảnh **sản phẩm** chỉ boot tới marker (chưa MMU/U-mode/core) — chạy shell sản phẩm cần **cả**
  đường `high_entry` **và** `Machine`, không chỉ ngân sách `.data+.bss`.

---

## 2. Trừu tượng hoá — một shim, hai backend

### 2.1 Cấu trúc

`vifast-plat-user/lib.rs` (839 dòng):

- `pub trait Sys` (dòng 69) với `const IPC_BUFFER_VA`, `invoke(nr, args[6])`,
  `ipc_send(nr, cap, info, mrs[4])`, `ipc_recv(nr, cap, reply)`, `yield_now()`, `exit()`;
- hai `mod` backend **chỉ** `#[cfg(target_arch)]`: `mod riscv64` (dòng 100, 56 dòng — `ecall`,
  `a7` nr, `a1` = `MessageInfo`) và `mod aarch64` (dòng 156, 54 dòng — `svc #0`, `x8` nr, `x2` =
  `MessageInfo`, MR0..3 `x3..x6`);
- mọi wrapper công khai (`syscall7`/`call`/`send`/`recv`/`exit`/`yield_now`/…) chỉ chuyển tiếp;
  **không** đổi chữ ký và **không** `cfg` trong thân ⇒ toàn bộ chương trình U-mode cũ biên dịch
  lại không sửa một dòng;
- `IPC_BUFFER_VA` = `<Backend as Sys>::IPC_BUFFER_VA` (riscv64 `0x4002_0000`, aarch64
  `0x7000_2000`).

### 2.2 Façade `syscall7` — chỗ ABI gửi khác nhau

`syscall7(nr, a0..a5)` giữ **một** chữ ký "cap + `MessageInfo` + MR0..3" cho chương trình:

- `nr ∈ {Call, Send, NBSend}` ⇒ gọi `Backend::ipc_send(nr, a0 (=cap), a1 (=info), [a2..a5])`;
  riscv64 đặt nguyên vị trí, aarch64 đặt `info` vào `x2` và MR0..3 vào `x3..x6`;
- các `nr` khác ⇒ `Backend::invoke(nr, [a0..a5])`.

Lượt chạy đầu (2026-11-23) chứng minh đây là **lỗi thật** nếu bỏ qua: `procsrv`/`shell` gọi
`syscall7` thẳng, nên bản đầu (chỉ `invoke`) đặt `info` ở `x1`; kernel đọc `x2` = MR0 ⇒ message
sai label/độ dài ⇒ **TAP 42/43 ĐỎ** (`up=0`, `events_help=0`) dù `send_err=0`. Sửa façade ⇒ xanh.

### 2.3 Cùng một nguồn Rust

- `procsrv`/`shell` chuyển từ `vifast-kbin-riscv64/init/` sang
  `v2/kernel/vifast-plat-user/programs/{procsrv,shell}.rs` (`git mv`, 178 + 1111 dòng);
  **cả hai** `build.rs` (riscv64 + aarch64) biên dịch **đúng hai tệp đó** cho target của mình —
  cùng một nguồn, khác target; **0 dòng `cfg`** trong thân hai chương trình.
- `programs/echo.rs` (75 dòng) chỉ aarch64: chương trình kiểm IPC buffer của TAP 44.
- Tỉ lệ "cùng nguồn": **2/2 chương trình lõi (1.289/1.289 dòng = 100 %)**, không có bản sao thứ
  hai; toàn bộ khác biệt arch nằm trong 110 dòng hai backend của shim.

---

## 3. aarch64 K3.5 — nạp ELF thật + ba TAP

### 3.1 Loader (`vifast-plat-aarch64/src/userload.rs`, 384 dòng)

- Dùng **lại** `vifast-elf::parse` (D2-26 — không có parser thứ hai): kiểm `e_machine=183`, cửa
  sổ user `0x7000_0000..0x7800_0000`, W^X từng segment (`PF_X` ⇒ `USER_RX`, `PF_W` ⇒ `USER_RW`,
  còn lại `USER_RO`); `W+X` bị parser từ chối.
- Mỗi trang: cấp frame từ arena boot, `zero_boot_ram` **trước** khi copy `filesz` (`.bss` đúng 0),
  `VSpace::map_obj_with`; đăng ký dải `[base,end)` để `SyscallEnv::put_str` chỉ đọc trong vùng đã
  map (fail-closed, nối tiếp KT-338b).
- `map_user_stack` (2 trang, đỉnh `0x7000_C000`) + `map_ring_with_script`: ring KT-69 hình học
  32×32, kernel bơm `help\necho same-source\nexit\n` (27 B) bằng **cùng** codec `conchan` mà shell
  đọc.
- LLD mặc định `p_align=64 KiB` cho aarch64 ⇒ ép `-zmax-page-size=0x1000` trong `build.rs` (parser
  chỉ nhận `align ≤ 4 KiB`, một Frame).

### 3.2 `k5user.rs` — điều phối ba pha, không scheduler

- Rig: CSpace 16 ô (6 Endpoint + 5 TCB + Frame IPC buffer + CNode), Untyped riêng 4 frame, ưu
  tiên tác vụ U-mode (10) > kern/server (1) để `Yield` quay lại chính nó.
- Pha procsrv: kernel đóng gói `WAIT_REQ`; mỗi `Yield` kernel rút `EP_EVENTS`/`EP_REPLY`, thấy
  `WAIT` xong ⇒ bơm `KILL_REQ`; thấy đủ 2 trả lời ⇒ `Running → Inactive` + `ELR_EL1 ← k5_shell`.
- Pha shell: ring đã có script; shell in prompt + `help` (`cmds=6`) + `echo` rồi `Exit(0)`;
  `Exit` ⇒ `ELR_EL1 ← k5_echo`.
- Pha echo: kernel prefill MR4..MR7 (`0xE100..0xE103`), chặn server `Recv`, echo đọc/ghi buffer
  (`^0x0000_00FF_0000_00FF`) rồi `send` message 8 MR; `Exit` ⇒ `k5_finish` đọc **thẳng** trang +
  `tcb_ipc_area(server)`.
- Không lưu/khôi phục khung trap: mỗi pha xong bỏ ngữ cảnh U-mode (cùng khuôn `Exit` KT-338b);
  **không** thêm scheduler/preemption.
- **Bằng chứng TAP 43 là kernel-side**: `vifast-plat-aarch64/src/k5cap.rs` (feature `ktest`) băm
  FNV-1a **đúng các byte** của mỗi lần `DebugPutStr`; TAP 43 đòi đủ bốn mốc
  `shell>`, `# shell: help cmds=`, `# shell: echo `, `# shell: exit code=0\n` (`transcript=0xf`).
  Vì sao không chỉ dựa hàng IPC: `Machine::exit` gọi `unlink_tcb_queues(tcb)` — message do TCB
  thoát gửi còn trong hàng Endpoint bị dọn; đo được `send_err=0` mà hàng rỗng ngay sau `Exit`
  (lượt 2026-11-23). Vì vậy TAP 43 kiểm **dữ liệu kernel đọc từ chính các lần in** + `puts=24` +
  `exit=0` + `state=Inactive`, không tin lời chương trình (D2-102).
- NP `BOOT_ARENA_FRAMES` **64 → 96**: K5 nạp 3 ELF (10 frame) + Untyped (4) + stack (2) + ring (1)
  + bảng trang mức 3; trên 64 frame đường PCI chết với
  `PANIC: arena boot hết frame cho trang DMA (KT-338a)` (đo được, không phải ước lượng). 96 =
  +32 frame (128 KiB RAM **ngoài** ảnh) — **không** đụng trần `.data+.bss`.

### 3.3 TAP mới — log QEMU nguyên văn

```
# k5 load: procsrv=0x71000000+4p entry=0x7100000c base=0x71000000 segs=3 shell=0x71200000+3p entry=0x712000d8 segs=2 echo=0x71400000+3p entry=0x714000a4 segs=2 stack=0x7000a000+8192 ring_pa=0x401c3000 ring_va=0x43000000 script=27B
# proc: up proto=wait/kill
# proc: wait pid=66 code=65535
# proc: kill pid=67 code=0
# k5 proc: up=1 wait_pid=66 wait_code=65535 kill_pid=67 kill_code=0 reply_wait=1 reply_kill=1 yields=22 timeout=0 takeover=1 root=10 state=Inactive
ok 42 - k5: procsrv serves 2 requests from EL0 (svc #0)
# shell: up cmds=help/echo/ps/wait/kill/exit
# shell: input=ring va=0x43000000
shell>
# shell: help cmds=6
  help | echo <text> | ps | wait <pid> | kill <pid> | exit
shell>
# shell: echo same-source
shell>
# shell: exit code=0
# k5 shell: exit=0 state=Inactive transcript=0xf puts=24 events_help=1 events_echo=1
ok 43 - k5: shell prompt + help/echo + clean exit from EL0
# echo: up
# echo: read mr4=0xe100 write mr4=0xff0000e1ff
# k5 echo: exit=0 state=Inactive len=8 reg0=0xe0a0 reg3=0xe0a3 buf0=0xff0000e1ff buf3=0xff0000e1fc got0=0xff0000e1ff got3=0xff0000e1fc prefill0=0xe100 mask=0xff000000ff
ok 44 - k5: echo EL0 reads/writes IPC buffer (length + content)
# k5 core: invariants=1
# viFastOs aarch64: boot ok
# viFastOs aarch64: halt
```

---

## 4. Quyết định ngân sách ảnh sản phẩm aarch64

**Đo (bắt buộc dán nguyên dòng)**:

```
# image=aarch64-product path=target/aarch64-unknown-none/release/vifast-kbin-aarch64 file=87408 arch=aarch64(e_machine=183) data+bss=0 budget=65536 stack=0 guard=0 smp_stacks=557056 noload=557056
```

`.data+.bss = 0 < 65.536` **nhưng** điều đó **không** đủ để bật `procsrv`/`shell` cho ảnh sản
phẩm:

1. K5 cần `vifast-core::Machine` để làm kernel syscall/IPC; `size_of::<Machine>() = 184.792 B`
   (TAP 36 in `size=184792 pin=196608`) — **lớn hơn** trần sản phẩm 65.536 B gần 3 lần.
   `soak-aarch64.sh`/D2-84 pin 196.608 B cho ảnh **kiểm**; D2-236/D2-244 **không** cho nới trần
   sản phẩm.
2. Ngoài `Machine`, rig K5 cần Endpoint/TCB/CNode + loader ELF (mã + bảng trang) + ring KT-69;
   ảnh sản phẩm hiện **chưa** bật `high_entry` (MMU/VSpace/GIC/U-mode) — bật shell sản phẩm là mở
   **cả đường boot**, không chỉ thêm một `mod`.

⇒ **Quyết định**: giữ K5 trong ảnh **kiểm** (`--features ktest`); ảnh sản phẩm aarch64 **không**
có TAP 42..44 và không có nhánh code K5 (mọi mục `#[cfg(feature = "ktest")]`). **Đề xuất task
con** (D2-245 — không DONE một phần):

> **KT-339a** — "Ảnh sản phẩm aarch64 có shell": (i) đưa `Machine` ra RAM **ngoài ảnh** (arena
> boot, như `BOOT_ARENA_FRAMES`) thay vì static `.bss`, đo lại `.data+.bss < 65.536`; (ii) bật
> `high_entry` (MMU/VSpace/GIC/PL011 RX) cho ảnh sản phẩm; (iii) nối UART RX → ring → shell; (iv)
> chứng minh TAP sản phẩm tương đương 42/43 trên ảnh không `ktest`.

---

## 5. Hai đột biến (bắt buộc, cả hai phải ĐỎ)

Cơ chế: `KTEST_FEATURES="ktest,mut-info-x1|mut-no-x0-out"` (runner `run-aarch64.sh` nhận biến
này — cùng một đường boot, không có script QEMU thứ hai); `build.rs` truyền `--cfg` tương ứng cho
shim `vifast_user_abi`.

| # | Đột biến | Kỳ vọng | Kết quả đo |
|---|---|---|---|
| (i) | `mut-info-x1`: `ipc_send` đặt `MessageInfo` ở `x1`, MR0..3 ở `x2..x5` (giả lập riscv64) | kernel aarch64 đọc `x2` = MR0 ⇒ message sai ⇒ **TAP 44 ĐỎ** (len/content) | **ĐỎ đúng**: TAP 44 `len=0`, `buf0=0x0`; kéo theo TAP 42 ĐỎ (`up=0`, hết ngân sách); TAP 43 vẫn xanh nhờ transcript |
| (ii) | `mut-no-x0-out`: `invoke` dùng `in("x0")` (không ghi error) | `procsrv` đọc `error` cũ ⇒ `NBRecv` thành công bị coi là lỗi ⇒ **TAP 42 ĐỎ** (timeout ngân sách) | **ĐỎ đúng**: TAP 42 `up=1 … wait_pid=0xFFFF…`, `yields=20000 timeout=1`; TAP 43/44 xanh |

---

Log nguyên văn (đột biến i):

```
# k5 proc: up=0 wait_pid=18446744073709551615 wait_code=18446744073709551615 kill_pid=18446744073709551615 kill_code=18446744073709551615 reply_wait=0 reply_kill=0 yields=20001 timeout=1 takeover=1 root=10 state=Inactive
not ok 42 - k5: procsrv serves 2 requests from EL0 (svc #0)
# k5 echo: exit=0 state=Inactive len=0 reg0=0x0 reg3=0x0 buf0=0x0 buf3=0x0 got0=0xff0000e1ff got3=0xff0000e1fc prefill0=0xe100 mask=0xff000000ff
not ok 44 - k5: echo EL0 reads/writes IPC buffer (length + content)
```

Log nguyên văn (đột biến ii):

```
# proc: up proto=wait/kill
# k5 proc: up=1 wait_pid=18446744073709551615 wait_code=18446744073709551615 kill_pid=18446744073709551615 kill_code=18446744073709551615 reply_wait=0 reply_kill=0 yields=20000 timeout=1 takeover=1 root=10 state=Inactive
not ok 42 - k5: procsrv serves 2 requests from EL0 (svc #0)
```

Cả hai đột biến chạy bằng **cùng** `run-aarch64.sh` với `KTEST_FEATURES="ktest,mut-…"` (không có
đường boot thứ hai); shim biên dịch lại với `--cfg mut_…` do `vifast-kbin-aarch64/build.rs`
truyền. Feature nằm ở `vifast-kbin-aarch64/Cargo.toml` (`mut-info-x1`, `mut-no-x0-out`).

## 6. Cổng

Bằng chứng runner aarch64 (nguyên văn, lượt `run-a64-23`):

```
ok 5 - boot 1: TAP 44/44 + marker 1 lần + hash ảnh khớp ELF
# ca âm: 27/27 ĐỎ đúng như phải
# 34/34 kiểm sạch (boot=1, tests=44)
```

TBD (`v2 check`, `gate --dev`, `ktest matrix`, `run-x86_64.sh`).

---

## 7. Nợ / đề xuất

- **Nợ 1** (đã nêu §4): ảnh sản phẩm aarch64 chưa có shell — cần KT-339a.
- **Nợ 2**: façade `syscall7` của shim mới dịch ABI gửi cho **một** cap (`Call`/`Send`/`NBSend`).
  `ReplyRecv`/`Call` 2–3 cap (MessageInfo ở `a3`/`a4` theo `spec/abi.toml`) chưa có chương trình
  aarch64 nào dùng; nếu mở, phải thêm nhánh dịch tương ứng + TAP.
- **Nợ 3**: ring UART→U-mode trên aarch64 vẫn do kernel **bơm script**; IRQ UART RX thật là nợ của
  KT-73 trên aarch64 (không thuộc K3.5).
- `BOOT_ARENA_FRAMES` 96: nếu thêm ảnh/thiết bị mới, đo lại `frames_used` trước khi tăng tiếp.
