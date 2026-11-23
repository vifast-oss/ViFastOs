# KT-345b — 28/28 parity: `free`/`df`/`loop` + `netsrv` đa caller (6A), rào chắn R1 D2-252

**Nhánh:** `task/kt345b` · **Base:** `main` @ `741cc840` · **Worktree:** `/private/tmp/kt345b-s1` ·
**Ngày:** 2026-11-24 (giờ máy Asia/Tokyo) · **Sub-agent:** `parity345b` · **Tệp journal duy nhất:** tệp này.
**Ngân sách:** 3 h; mốc cứng 08:15 JST. Quyết định: **D2-252** (rào chắn Pha A), **D2-250**,
**D2-249**, **D2-247**, **D2-245**, **D2-243**, **D2-233**, **D2-05** (test đỏ trước).
Nhãn số liệu: **`[ĐO KT-345b]`** — đo từ log QEMU/host test trong worktree này.

## 0. Chốt một câu

Lật **3 mục `deferred-K7` cuối** (`free`, `df`, `loop`) → `active` ⇒ `spec/v1-parity.toml`
**28/28**; parity `--mode serial` trên **ảnh sản phẩm** **28/28** ở `smp=1` và `smp=4`. Ba lệnh
được cài **trong `shell_cmds.rs`** (rào chắn R1: `shell.rs` **+1 dòng nối**, `core_ipc.rs`
**0 dòng**); phía kernel là **hai dịch vụ mới** (`mem_svc.rs`, `loop_svc.rs`) + `OP_STATFS` của
`proc_fs.rs` — **không** mở ABI syscall. `netsrv` phục vụ **2 caller** (shell + procsrv) trên
**hai cặp endpoint**, bảng phiên có biên + handle, `direct_pushed` cho **cả hai**, không mất
message. **Hai đột biến ĐỎ**; `product-riscv64.sh` **fails=0**, **0 marker**; `size_of::<Machine>()`
**184.792 B** không đổi; `.data+.bss` sản phẩm riscv64 trong trần **262.144 B**.

## 1. Khảo sát + kế hoạch R1 (đo `wc -l` trước/sau)

### 1.1 Ba lệnh cần gì — opcode/syscall nào

| Lệnh | Dịch vụ | Request/reply | Nguồn số liệu **thật** | Tệp |
|---|---|---|---|---|
| `free` | `MemSvc` của kernel (`meminfo.vidl` 4C) | `NBSend(EP_MEM, MEM_REQ)`, reply `MEM_REPLY` trên `EP_REPLY` | `Σ Untyped.capacity` và `total_used()` của **core object layer** (`.corespace`: TCB/SC/Endpoint) | `mem_svc.rs` (mới), `shell_cmds.rs` |
| `df` | `FsSvc` của kernel (`OP_STATFS`, `fsstat.vidl` 4C) | `NBSend(EP_FS, FS_REQ\|OP_STATFS)`, reply `FS_REPLY` | `Fs::usage()` = số block **vùng dữ liệu** (`capacity − DATA_START`) | `proc_fs.rs`, `shell_cmds.rs` |
| `loop` | `LoopSvc` của kernel (D2-252) | `NBSend(EP_LOOP, LOOP_REQ)` | mỗi vòng **Retype Tcb+SC → Configure/Start → Delete+Revoke** từ Untyped 4 KiB riêng; `used` trước/sau của `Machine::total_used()` | `loop_svc.rs` (mới), `shell_cmds.rs` |

**Không** thêm syscall/ABI: cả ba đi trên `NB_SEND`/`NB_RECV` đã có (đúng khuôn `proc_fs`/`net_svc`
của KT-77c/KT-306), nên **không** phải chạm `core_ipc.rs`.

### 1.2 Dòng mỗi tệp (trước → sau)

```text
$ wc -l v2/kernel/vifast-plat-user/programs/{shell_cmds,shell}.rs v2/kernel/vifast-core/src/core_ipc.rs
     243 shell_cmds.rs          →  363  (+120: toàn bộ 3 lệnh + helper + dispatch)
    1414 shell.rs               → 1415  (+1: đúng MỘT dòng nối `extra_dispatch(line)`; CMDS/help
                                          sửa trên dòng có sẵn, không thêm dòng logic)
    1498 core_ipc.rs            → 1498  (+0: KHÔNG chạm — rào chắn D2-252)
```

`shell.rs` 1415/1.500, `core_ipc.rs` 1498/1.500 ⇒ **R1 không bị nới**, thân ba lệnh nằm trọn
trong `shell_cmds.rs`.

### 1.3 Nguồn số liệu — vì sao **không** bịa

- `free`: `MemSvc::measure` đếm `object_count()` rồi cộng `untyped_size(id)` của mọi Untyped còn
  sống trong **core Machine** (`.corespace`) và `total_used()` là tổng watermark. Đây **đúng đại
  lượng** `loop` đo `before`/`after` ⇒ người dùng chạy `free`, `loop 6`, `free` phải thấy cùng
  số (bằng chứng chống rò rỉ ngay trên máy, như v1 `36/344` trước/sau `loop 6`).
- `df`: `Fs::usage()` là O(1) trên `used_blocks` (được `mark_used`/`mark_free` duy trì), đã có
  host test `usage_tracks_allocation_and_unlink`; KT-345b thêm ca host khoá **đúng** phương tiện
  của `--mode serial` (volume RAM 96 block ⇒ `6/66`).
- `loop`: spawn/destroy **thật** một TCB + `SchedContext` mỗi vòng, không phải đếm suông. Untyped
  **riêng** 4 KiB là bắt buộc: `Revoke` một Untyped finalise **cả cây** object của nó — dùng chung
  U2 sẽ thổi bay endpoint/TCB của cả ảnh.

## 2. Thay đổi (tệp + dòng)

| Tệp | +/− | Nội dung |
|---|---:|---|
| `spec/v1-parity.toml` | +27/−27 | **3 mục `deferred-K7` → `active`** (`free`/`df`/`loop`) với `expect` regex thật của v2 + `note`; tổng vẫn **28 mục**, `deferred=0` |
| `v2/kernel/vifast-plat-user/programs/shell_cmds.rs` | **+120** | `cmd_free`/`cmd_df`/`cmd_loop` + `reply_call` (hạn `FS_REPLY_BUDGET`) + `extra_dispatch`; hằng giao thức khớp `proc_abi` |
| `v2/kernel/vifast-plat-user/programs/shell.rs` | **+1** | một dòng nối `extra_dispatch(line)` ở cuối `fs_dispatch`; `CMDS` 25→28 + danh sách `help`/`up` (sửa dòng có sẵn) |
| `v2/kernel/vifast-kbin-riscv64/src/mem_svc.rs` | **+92 (mới)** | `MemSvc`: endpoint `EP_MEM` (ô 21), `total`/`free` từ Untyped của core |
| `v2/kernel/vifast-kbin-riscv64/src/loop_svc.rs` | **+154 (mới)** | `LoopSvc`: endpoint `EP_LOOP` (ô 22) + Untyped 4 KiB (ô 23); mỗi vòng Retype Tcb+SC → Start → Delete+Revoke; `ok` chỉ khi `total_used()` trước = sau; `n ≤ 32` |
| `v2/kernel/vifast-kbin-riscv64/src/proc_fs.rs` | +16/−1 | `OP_STATFS` → `statfs` (`[ST_OK, used, total, block_size]`) |
| `v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs` | +55 | dựng hai dịch vụ trong `Rig`, gọi trong `service`; **`direct_pushed_proc`** = `pushed` của hàng reply caller 2 |
| `v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs` | +41 | `EP_MEM`/`EP_LOOP`/`EP_NET_CLIENT`/`EP_NET_CLIENT_REPLY`, `LOOP_ARENA`/2 ô nháp, `OP_STATFS`, `MEM_*`/`LOOP_*` |
| `v2/kernel/vifast-kbin-riscv64/src/proc_net.rs` | +25 | **caller thứ hai**: endpoint `EP_NET_CLIENT`/`EP_NET_CLIENT_REPLY` (ô 26/27), copy cap vào cnode `netsrv` ô 5/6 |
| `v2/kernel/vifast-kbin-riscv64/init/netsrv_prod_user.rs` | +235/−48 | vòng lặp **đa caller** trên 2 kênh, **bảng phiên 4 ô** `(client, handle)`: `handle=0` mở/dùng lại, lạ ⇒ `InvalidCapability`(1), đầy ⇒ `Busy`(14); **không** thoát sau 1 request |
| `v2/kernel/vifast-plat-user/programs/procsrv.rs` | +36 | `--cfg product_net`: procsrv là **caller thứ hai** (ping lúc boot + in reply); ảnh test không đổi byte ELF |
| `v2/kernel/vifast-kbin-riscv64/build.rs` | +17 | truyền `--cfg product_net` cho `procsrv.elf`; truyền `--cfg mut_net_one_caller` khi bật feature đột biến |
| `v2/kernel/vifast-kbin-riscv64/Cargo.toml` | +5 | feature đột biến `mut-df-round-up`, `mut-net-one-caller` (không dùng ở ảnh nào) |
| `v2/kernel/vifast-kbin-riscv64/src/main.rs` | +6 | `mod mem_svc; mod loop_svc;` (cfg `product-fs`, khai ở cuối tệp) |
| `v2/kernel/vifast-core/src/kt345b_tests.rs` | **+120 (mới)** | 2 host test: loop spawn→destroy **2 vòng** trả byte về mốc; bỏ `Revoke` ⇒ watermark tăng (ca đối chứng) |
| `v2/kernel/vifast-core/src/lib.rs` | +4 | `#[cfg(test)] mod kt345b_tests;` |
| `v2/lib/vifast-fs/tests/kt345b_statfs_host.rs` | **+107 (mới)** | host test `usage()` = `(6,66)` trên volume RAM 96 block + seed đúng `proc_fs::seed` (khoá số của `df`) |
| `v2/test/boot/product-riscv64.sh` | +39 | phép kiểm `free`/`df`/`loop`, `df` **khớp** dòng `# product fs:` (cùng nguồn số), `loop` **used trước = sau**, 2 caller `client=1`/`client=2`, `direct_pushed_proc=1 direct_pushed=1` |

Không chạm: `spec/abi.toml`, `spec/kernel.toml`, `spec/protocols/*.vidl` (đọc-only ở task này),
`v2/agent/{STATE,QUEUE,DECISIONS}.md`, `v2/bin/**`, `.github/**`, 19 tệp `[[legacy]]`, `main`.

## 3. Bằng chứng chạy thật (ảnh sản phẩm riscv64)

### 3.1 Ba lệnh trên một boot sạch (QEMU `virt`, **không** `-drive` ⇒ volume RAM 96 block)

```text
# shell: up cmds=help/echo/ps/wait/kill/ls/cd/pwd/cat/write/time/uptime/sync/stat/truncate/jobs/mkdir/rm/mv/ln/fsck/uname/free/df/loop/ping/run/runfs/exec/exit
# product fs: medium=ram blocks=96 used=6 total=66 volatile=1 why=khong-co-pci const:ram_pa=0x83000000 …
# shell: free 55/68 KiB
# shell: df 6/66 block
# shell: loop n=6 LOOP OK used=12352/12352
```

- `free`: **55/68 KiB** — `total` = tổng Untyped sống của core object layer; `free` = phần còn
  cấp được. Hai số **cùng đại lượng** với `loop` ⇒ `used` trước/sau `loop 6` **bằng nhau**
  (12352 B = 12352 B) — đúng phép kiểm chống rò rỉ của v1 (`free` 36/344 trước và sau `loop 6`).
- `df`: **6/66 block** — `total` = 96 − `DATA_START`(30); `used` = 3 thư mục + 3 tệp của
  `proc_fs::seed`. Khớp **cùng nguồn số** với dòng `# product fs: … used=6 total=66` (host test
  `kt345b_statfs_host` khoá đúng cặp số này).
- `loop`: **LOOP OK**, `used` trước = sau (không rò rỉ) sau **6 vòng** spawn→destroy TCB thật.

### 3.2 `netsrv` đa caller (6A) — 2 caller, không mất message

```text
# product net: served=1 … sent=1 received=1 label=0x291 caller=28 direct_pushed_proc=0 direct_pushed=0 fwd=0
# netsrv: ping 10.0.2.2 rtt_ms=6 err=0 handle=1 client=2
# netsrv: reply nb err=0
# product net: served=1 … direct_pushed_proc=1 direct_pushed=0 fwd=0
# proc: ping 10.0.2.2 rtt_ms=6 err=0 handle=1        ← caller thứ hai (procsrv, kênh EP_NET_CLIENT)
# shell: free 55/68 KiB
# shell: df 6/8162 block                              ← boot này có `-drive` 4 MiB: 8192 − 30
# shell: loop n=6 LOOP OK used=12352/12352
# product net: served=2 … sent=2 received=2 … direct_pushed_proc=1 direct_pushed=0 fwd=0
# netsrv: ping 10.0.2.2 rtt_ms=0 err=0 handle=2 client=1
# netsrv: reply nb err=0
# product net: served=2 … direct_pushed_proc=1 direct_pushed=1 fwd=0
# shell: ping 10.0.2.2 rtt_ms=0 err=0                 ← caller 1 (shell, kênh EP_NET)
```

Đọc: hai **caller TCB** khác nhau (`procsrv`, `shell`) trên hai cặp endpoint; `netsrv` cấp
`handle=1`/`handle=2`, phục vụ **cả hai** và trả lời **trực tiếp** vào hàng reply của từng caller
(`direct_pushed_proc=1` và `direct_pushed=1`, `fwd=0` — kernel **không** chuyển tiếp). Server
**không** thoát sau một request nữa (nợ KT-342 đã đóng).

### 3.3 Parity `--mode serial` **28/28** trên ảnh sản phẩm (`smp=1`)

```text
$ bash v2/test/parity/run.sh --mode serial --image product --smp 1 --timeout-ds 60
# parity: spec_total=28 spec_active=28 deferred=0 extra=0 total=28 (nguon: spec/v1-parity.toml)
ok 1 - help … ok 26 - uname / ok 27 - history / ok 28 - !!
# parity: total=28 ok=28 deferred=0 active=28 spec_total=28 extra=0 (active_da_chay=28)
# parity: not_ok=0 timeout=0 khong_co_shell=0 qemu_rc=124
# parity: KET_LUAN=ok (exit 0)
```

`smp=4`: xem §5 (bảng cổng) — cùng 28/28.

### 3.4 Parity `--mode serial` **28/28** trên ảnh sản phẩm (`smp=4`)

```text
$ bash v2/test/parity/run.sh --mode serial --image product --smp 4 --timeout-ds 60
# parity: spec_total=28 spec_active=28 deferred=0 extra=0 total=28 (nguon: spec/v1-parity.toml)
ok 1 - help … ok 27 - history / ok 28 - !!
# parity: total=28 ok=28 deferred=0 active=28 spec_total=28 extra=0 (active_da_chay=28)
# parity: not_ok=0 timeout=0 khong_co_shell=0 qemu_rc=124
# parity: KET_LUAN=ok (exit 0)
```

**25 → 28**: ba mục cuối (`free`/`df`/`loop`) đều `ok` ở **cả hai** cấu hình SMP; `deferred=0`.

## 4. `netsrv` đa caller (6A) — hợp đồng đã cài

`spec/protocols/socket.vidl` (D2-242 nhóm 6A) chốt: **bảng phiên có biên**, danh tính caller,
đầy ⇒ `Busy`(14), sai danh tính/handle ⇒ `InvalidCapability`(1), **kernel `NetSvc` giữ đơn
caller**. Ảnh sản phẩm chưa mint `badge` cho client, nên **danh tính caller = cặp endpoint** mà
request đến (hai cặp endpoint riêng, không thể giả mạo chéo); `handle` do server cấp đơn điệu.

| Thành phần | Trước KT-342b | Sau KT-345b |
|---|---|---|
| Server | phục vụ **một** request rồi `Exit(0)` | vòng lặp phục vụ **đến hết** request trên **2** kênh |
| Caller | `shell` (`EP_NET`) | `shell` (`EP_NET`) **+** `procsrv` (`EP_NET_CLIENT`) |
| Phiên | không | bảng **4 ô** `(client, handle)`; `handle=0` mở/dùng lại; lạ ⇒ 1; đầy ⇒ 14 |
| Kernel `NetSvc` | đơn caller (`Reply.caller` = TCB `netsrv`) | **giữ nguyên đơn caller** (chỉ kiểm quyền, không đổi) |
| Trả lời | `EP_REPLY` của kernel (trước KT-342b) → trực tiếp `EP_NET_REPLY` | trực tiếp **cả hai** hàng `EP_NET_REPLY`/`EP_NET_CLIENT_REPLY` (`direct_pushed_proc=1`, `direct_pushed=1`, `fwd=0`) |

Hai caller **không** tranh message: mỗi kênh có hàng riêng; server phục vụ tuần tự từng message
nhưng **không** dừng sau caller đầu. Bằng chứng trên **một** boot: `client=2` (procsrv) rồi
`client=1` (shell), `handle` 1 và 2, cả hai nhận `rtt_ms`/`err=0` (§3.2).

**Nợ 6A còn lại (nói thẳng)**: socket.vidl còn `connect`/`send`/`recv`/`poll`/`close` + cap
`Frame` — task này chỉ cài phần **đa caller + bảng phiên + ping** mà DoD `KT-345b` yêu cầu, và
đúng phần kernel `NetSvc` được phép chạm. Phần còn lại **không** thuộc DoD này; đề xuất task con
`KT-345c` (2–3 h) khi mở TCP thật (N1) — không nhận "DONE một phần" cho phần đã cài.

## 5. Cổng (T0/T1 + runner) — số đo trên cây **đã commit** (build-inputs `dirty=0`)

| Cổng | Kết quả |
|---|---|
| `./v2/bin/v2 check` | **PASS** — `TỔNG check 117.053 ms`; `gen-check`/`reserved-check`/rules PASS |
| `./v2/bin/v2 gate --dev` (T1) | **PASS** — `TỔNG gate --dev (T1) 768.889 ms`; in rõ "KHÔNG phải full gate" |
| `bash v2/test/parity/run.sh --mode serial --smp 1` (ảnh sản phẩm) | **28/28** — `total=28 ok=28 deferred=0 active=28`, `KET_LUAN=ok` |
| `… --smp 4` | **28/28** — cùng dòng tổng kết, `KET_LUAN=ok` |
| `bash v2/test/boot/product-riscv64.sh` | **exit 0** — `fails=0`, **rev-gate ACCEPT (D2-125.3)**, marker **0** (D2-08), budget `data+bss=224472 budget=262144` |
| `bash v2/test/boot/ktest.sh 1 matrix` | **27/27 boot sạch** (`FAIL=0 TIMEOUT=0`, 9 ảnh × smp 1/2/4), `ktest.sh: PASS` |
| `bash v2/test/boot/run-aarch64.sh` | **37/37 kiểm sạch**, `tests=46` (giữ nguyên TAP aarch64) |
| `bash v2/test/boot/run-x86_64.sh` | **37/37 kiểm sạch**, `tests=47 exc=6` (giữ nguyên TAP x86_64) |
| Host test `vifast-core` (`cargo test -p vifast-core kt345b`) | **2/2 PASS** (spawn→destroy 2 vòng; ca đối chứng thiếu `Revoke`) |
| Host test `vifast-fs` (`cargo test --test kt345b_statfs_host`) | **1/1 PASS** (`usage()=(6,66)` trên volume RAM 96 block) |
| `cargo fmt --all -- --check` (kernel + vifast-fs) | **sạch** |
| `cargo clippy --workspace --all-targets -- -D warnings` | **PASS** |

Ràng buộc hiến định **không đổi** (đo từ log product):

```text
# core: product addr=0x8024a0b8 bytes=184792 pin=196608 image_cap=262144 … footprint_ok=1
ok  - budget: … data+bss=224472 budget=262144 …
ok  - markers: 0 harness marker (4 mẫu) — D2-08
```

`size_of::<Machine>()` = **184.792 B** (không đổi; không chạm `machine.rs`), `.data+.bss` sản
phẩm riscv64 = **224.472 B ≤ 262.144 B** (headroom 37.672 B, bằng KT-343), **0 marker** harness.

**R1 — chứng minh không nới** (`wc -l`, cây đã commit):

```text
$ wc -l v2/kernel/vifast-plat-user/programs/{shell_cmds,shell}.rs v2/kernel/vifast-core/src/core_ipc.rs
     363 …/shell_cmds.rs     (243 → +120: toàn bộ thân 3 lệnh + helper + dispatch)
    1415 …/shell.rs          (1414 → +1: đúng MỘT dòng nối `extra_dispatch(line)`)
    1498 vifast-core/src/core_ipc.rs   (1498 → +0; `git diff 741cc840 -- core_ipc.rs` rỗng)
$ git diff --numstat 741cc840 -- v2/kernel/vifast-core/src/core_ipc.rs
(không có dòng nào)
```

## 6. Hai đột biến **ĐỎ** (D2-05)

| # | Đột biến | Cách làm | Kết quả **đỏ** (log) |
|---|---|---|---|
| i | `df` trả số block sai (**làm tròn lên**) | feature `mut-df-round-up`: `total = (total + 7) & !7` trong `proc_fs::statfs` | parity mục `df` (pinned `^# shell: df 6/66 block$`) **`not ok`**: log thật `# shell: df 6/72 block` ⇒ `KET_LUAN=timeout (exit 4)` |
| ii | `netsrv` chỉ phục vụ **1 caller** | feature `mut-net-one-caller` → `--cfg mut_net_one_caller` cho `netsrv_prod.elf`; `exit(0)` sau reply đầu | chỉ `client=2` được phục vụ, **`client=1` = 0 lần**; `# shell: ping timeout`; **không** có `direct_pushed=1` ⇒ 3 phép kiểm đa caller của `product-riscv64.sh` **đỏ** (đã kiểm bằng chính regex: `CLIENT1=no`, `BOTH=no`, `SHELL=no`) |

```text
# (i)  $ cargo build … --features product-all,mut-df-round-up
#      $ bash v2/test/parity/run.sh --mode serial --image product --smp 1 --extra mut-df.toml --extra-only
not ok 1 - df: timeout sau 4000 ms — expect khong xuat hien du 1 lan (^# shell: df 6/66 block$)
# shell: df 6/72 block
# parity: KET_LUAN=timeout (exit 4)

# (ii) $ cargo build … --features product-all,mut-net-one-caller   (boot -netdev user)
# netsrv: ping 10.0.2.2 rtt_ms=5 err=0 handle=1 client=2
# proc: ping 10.0.2.2 rtt_ms=5 err=0 handle=1
# shell: ping timeout
```

Sau hai đột biến, ảnh sạch được dựng lại `--features product-all` (cây nguồn **không** đổi — đột
biến là **feature**, không phải sửa mã) và **parity 28/28** chạy lại trên ảnh sạch (§5).

## 7. Nợ / đề xuất (task con hợp lệ)

1. **`KT-345c` (đề xuất, 2–3 h)** — phần **còn lại** của `spec/protocols/socket.vidl` 6A:
   `connect`/`send`/`recv`/`poll`/`close` + cap `Frame` cho `send`/`recv`, TCP (N1) thật. Task này
   đã đóng phần DoD: **đa caller + bảng phiên có biên + handle + fail-closed**; không nhận thêm.
2. **Ngân sách trang ELF shell** (phát hiện trong task, phải nói): shell U-mode riscv64 kết thúc
   tại `0x4000_6f58`, guard stack ở `0x4000_7000` ⇒ **còn 168 B** (trước task: 368 B). `shell.rs`
   đã cố ý chỉ tăng **1 dòng** và thân lệnh đặt trong `shell_cmds.rs`, nhưng **mọi** lệnh mới vẫn
   nằm trong **cùng một ELF** (ngân sách trang ~28 KiB) ⇒ lần thêm lệnh tiếp theo rất dễ vượt.
   Đề xuất: `KT-346` (tách userland) hoặc task con `KT-345d` **đo + tách ngân sách trang shell**
   trước khi thêm lệnh; **không** nới guard/stack để lấy chỗ.
3. `free` đo **object layer của core** (`.corespace`: TCB/SC/Endpoint) — đúng đại lượng `loop`
   dùng, nhưng **không** phải toàn bộ DRAM/FrameArena của user object layer. Muốn "free RAM" vật
   lý thật cần API thống kê trên `vifast_alloc::FrameArena` (ngoài phạm vi task; đề xuất khi làm
   `U1`/`KT-347`).
4. `df` ghim `6/66` cho chế độ **volume RAM 96 block** (`--mode serial` không `-drive`); nếu
   `proc_fs::seed` đổi số tệp/thư mục thì phải cập nhật `expect` **cùng lúc** (hợp đồng có chủ ý,
   `kt345b_statfs_host` khoá cùng cặp số).
5. `loop <n>` chặn `n ≤ 32` (cùng ngưỡng v1); `n > 32` bị kẹp **âm thầm** như v1 — nếu muốn báo
   lỗi rõ thì mở ở task sau (không đổi parity hiện tại).

## 8. Commit

- `839c44e2` `[KT-345b] free/df/loop 28/28 parity + netsrv đa caller (6A); R1: shell_cmds 243→363, shell.rs 1414→1415, core_ipc 1498`
- Journal (tệp này) commit riêng `[KT-345b]` — nằm trong `v2/agent/**` nên **không** thuộc
  build-inputs (D2-127); bằng chứng cổng ở §5 vẫn hợp lệ (dấu vân tay build không đổi).
- **Không** push/merge/`main`; cây làm việc sạch sau commit journal.
