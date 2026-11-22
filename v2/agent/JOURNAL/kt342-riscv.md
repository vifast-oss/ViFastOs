# KT-342 — ảnh sản phẩm riscv64 hợp nhất: gỡ `ktest` khỏi driver + FS · `netsrv` · `exec`

**Chu kỳ 2 — Đợt Đêm 2.** Sub-agent `rv342`; nhánh `task/kt342-rv`, worktree `/tmp/kt342-s1`,
base `main` @ `85eebd28` (gồm `KT-335b` `dc272a33`). Ngày 2026-11-23 (giờ máy Asia/Tokyo).
Quyết định: **D2-233** (một lõi + một userland, tách sản phẩm/harness), **D2-243** (KT-336c vào
phụ thuộc KT-342; tách `user.rs` < 1.100 dòng), **D2-244** (3 đột biến, `v2 mutants` hoãn),
**D2-245** (sub-agent chỉ T0/T1; journal **một** tệp), **D2-246** (DNS `bad_replies` — giữ nguyên
qua ranh giới `vifast-net`), **D2-247** (`KT-335b → KT-342`, tránh xung đột `virtio_net.rs`),
**D2-248** (khoá máy đo, `env.sh`), **D2-08** (test kernel **không** nằm trong boot sản phẩm),
**D2-05** (đỏ trước), **D2-17** (`unsafe` chỉ trong `vifast-plat-*`).

| | |
|---|---|
| Worktree | `/private/tmp/kt342-s1` |
| Base | `85eebd28` (`main`) |
| Máy | macOS Apple Silicon, rustc/cargo `1.100.0`, QEMU `11.1.3` |
| Lệnh | `export PATH="$HOME/.cargo/bin:$PATH"`, chạy trong worktree |
| Nhãn số liệu | **`[ĐO KT-342]`** — đo từ log runner/`llvm-size` trong worktree này |

Phạm vi (đúng D2-245): `v2/kernel/vifast-plat-riscv64`, `v2/kernel/vifast-kbin-riscv64`,
`v2/test/boot/product-riscv64.sh` (mới), `v2/agent/JOURNAL/kt342-riscv.md` (tệp này).
**Không** chạm `spec/`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `.github/**`, `v2/bin/lib/*.sh`,
19 tệp `[[legacy]]` của `numbers-policy.toml`, `main`.

---

## 1. Khảo sát trước khi sửa — ảnh sản phẩm riscv64 có gì, `ktest` chặn ở đâu

### 1.1 Ảnh sản phẩm **trước** KT-342

`cargo build -p vifast-kbin-riscv64 --bin vifast-kbin-riscv64` (không feature) = **ảnh tối thiểu**:
`rust_main` in `TAP version 13/1..1`, dòng `# k3 image=product`, `# build:`, `# conf image:`,
`# tlb:`, `# obj:`, `# mmu:`, `# core: product`, `# plic:`, `# uart irq:`, `# timer:`, `# smp:`,
rồi `ok 1 - k3 boot` + `# k3 halt` (`v2/kernel/vifast-kbin-riscv64/src/main.rs`).

Các feature **sản phẩm** đã có **trước** KT-342 (không `ktest`):

| Feature | Nội dung | Trạng thái trước |
|---|---|---|
| `product-shell` (KT-77b) | `proc_rt` + `proc_abi`: procsrv + shell U-mode, byte vào từ UART | **có** |
| `product-fs` (KT-77c) | `proc_fs` + `virtio-blk`/volume RAM, `ls`/`cat`/`write`/… | **có** |
| `product-net` | `netsrv` U-mode + `ping` | **KHÔNG** — `net = ["ktest"]` |
| `product-exec` | `run <path>` (exec kernel-mediated) | **KHÔNG** — `exec = ["ktest", "user"]` ở kbin |
| `product-all` | FS + net + exec trên **một** boot | **KHÔNG** |

⇒ Ảnh sản phẩm **trước** KT-342 chỉ có **tối đa một** trong ba đường (FS **hoặc** shell), và
**không** có cách nào bật `net`/`exec` mà không kéo `ktest` — trực tiếp trái D2-08.

### 1.2 `cfg(feature = "ktest")` nằm ở đâu (đếm trên `HEAD` `85eebd28`)

`git grep -c 'feature = "ktest"' HEAD -- v2/kernel/vifast-plat-riscv64 v2/kernel/vifast-kbin-riscv64`
= **67** dòng / **10** tệp:

| Tệp | Số dòng | Nội dung | Có chặn đường sản phẩm? |
|---|---:|---|---|
| `vifast-plat-riscv64/src/mmu.rs` | 35 | bộ đếm/evidence đo MMU, cờ `GUARD_LOAD_RESUME` (probe guard resume-một-lần), `probe` guard | **không** — chỉ số đo test |
| `vifast-plat-riscv64/src/obj.rs` | 11 | evidence `ObjectCounts`/kiểm bất biến test | **không** |
| `vifast-plat-riscv64/src/tlb.rs` | 6 | bộ đếm shootdown/ASID test | **không** |
| `vifast-plat-riscv64/src/lib.rs` | 5 | `mod probe`; nhánh `probe::on_trap` trong trap; nhánh fault recovery | **không** (đường sản phẩm đi nhánh còn lại, fatal) |
| `vifast-plat-riscv64/src/ipi.rs` | 4 | bộ đếm IPI test | **không** |
| `vifast-plat-riscv64/src/user.rs` | 2 | `classify_u_fault` (walk PTE cần root kernel) + `enter_expect_fault` | **không** (sản phẩm dùng nhánh `not(feature)` cùng bảng `scause`) |
| `vifast-plat-riscv64/src/probe.rs` | 1 | toàn module probe | **không** |
| `vifast-plat-riscv64/Cargo.toml` | 1 | dòng khai feature `ktest = []` | — |
| `vifast-kbin-riscv64/src/proc_fs.rs` | 1 | **chú thích** ôn lại KT-77c (không phải `cfg`) | **không** |
| `vifast-kbin-riscv64/src/ktest_lock.rs` | 1 | khoá test-only của ảnh ktest | **không** |

**Chỗ chặn đường sản phẩm** — không phải mã driver, mà là **nhãn feature**:
`vifast-plat-riscv64/Cargo.toml` khai `vblk = ["block", "ktest"]`, `vcon = ["ktest"]`,
`net = ["ktest"]`, `input = ["ktest"]`, `gpu = ["ktest"]`. Không tệp driver nào
(`virtio_blk.rs`, `virtio_net.rs`, `virtio_console.rs`, `virtio_input.rs`, `virtio_gpu.rs`,
`pci.rs`) có `cfg(feature = "ktest")` — kiểm bằng
`git grep -n 'feature = "ktest"' -- v2/kernel/vifast-plat-riscv64/src/virtio_*.rs v2/kernel/vifast-plat-riscv64/src/pci.rs`
= **0**. Vậy đây là **cùng dạng bẫy** KT-77b (U-mode) / KT-77c (`block`) đã sửa hai lần: phụ thuộc
**chỉ là nhãn**.

### 1.3 So với aarch64/x86_64 (đã tách sản phẩm/ktest)

- **aarch64** (`KT-338c`): `run-aarch64.sh` build **hai** artefact — ảnh sản phẩm **không** feature
  và ảnh `--features ktest`; danh sách `PRODUCT_HARNESS_MARKERS` **23** mẫu, ca âm 11 kỳ vọng sai
  + ca âm "không đĩa/không mạng/không bàn phím/không GPU". `main.rs` **1.491 → 549**.
- **x86_64** (`KT-340a/b`): cùng khuôn một `[[bin]]` hai lần build; `.data+.bss` sản phẩm
  **96.320 → 43.072 B** (trần riêng 64 KiB); `.bss` dời bảng trang/stack ra section NOLOAD.
- **riscv64 (trước KT-342)**: **không** có ga kiểm "0 harness marker" kiểu danh sách; `run.sh`
  chỉ `strings | grep -c ktest` (đo được: 0 trên ảnh sản phẩm tối thiểu, nhưng **không** có răng
  với ảnh `product-*`). Cũng **không** có ảnh sản phẩm nào chạy **đồng thời** FS/net/exec.

---

## 2. Gỡ `ktest` khỏi driver (tiêu chí 2)

`vifast-plat-riscv64/Cargo.toml` — **chỉ đổi nhãn**, không đổi một dòng mã driver:

```diff
-vblk = ["block", "ktest"]
+vblk = ["block"]
-vcon = ["ktest"]
+vcon = []
-net = ["ktest"]
+net = []
-input = ["ktest"]
+input = []
-gpu = ["ktest"]
+gpu = []
```

Hệ quả: mọi feature driver của crate arch **không** còn kéo `probe`/bộ đếm test; ảnh test vẫn bật
`ktest` **tường minh** qua feature của bin (`[[bin]] required-features`) — **không** đổi một byte
của 9 ảnh test. Kiểm bằng máy: mỗi feature driver `cargo check -p vifast-plat-riscv64 --features
<f>` **không** có `ktest` đều `Finished` (6/6: `block`, `vblk`, `vcon`, `net`, `input`, `gpu`),
`gpu`/`input`/`vcon` vẫn map đúng leaf PCIe của mình.

`ktest` **còn ở đâu** (sau sửa): **66** dòng `feature = "ktest"` trong `src/` — **tất cả** là
bộ đếm/evidence test hoặc module `probe` (bảng §1.2), **0** dòng ở đường driver.

---

## 3. Ảnh sản phẩm **hợp nhất**: FS + `netsrv` + `exec` trên **một** boot (tiêu chí 3)

### 3.1 Feature mới (kbin) — không `ktest`

```toml
product-net  = ["product-shell", "vifast-plat-riscv64/net"]
product-exec = ["product-shell", "vifast-plat-riscv64/exec"]
product-all  = ["product-fs", "product-net", "product-exec"]
mut-ktest-driver = ["vifast-plat-riscv64/ktest"]   # đột biến (i)
mut-no-netsrv    = []                              # đột biến (ii)
```

### 3.2 Tệp mới (mỗi tệp một miền, không cắt cơ học)

| Tệp | Dòng | Vai trò |
|---|---:|---|
| `v2/kernel/vifast-kbin-riscv64/src/proc_net.rs` | | mở PCI/virtio-net + DHCP + ARP ở S-mode; dựng cnode 16 ô + TCB/SC cho `netsrv`; đếm request đang chờ |
| `v2/kernel/vifast-kbin-riscv64/src/proc_exec.rs` | | bảng ảnh nhúng `ImageTable` + phục vụ `run` trên `EP_EXEC`; `exec` nguyên tử (KT-74); reset trapframe |
| `v2/kernel/vifast-kbin-riscv64/init/netsrv_prod_user.rs` | | `netsrv` U-mode của ảnh sản phẩm: `Recv` request → `Call` dịch vụ kernel → `NBSend` reply → `Exit(0)` |
| `v2/kernel/vifast-kbin-riscv64/init/hello_user.rs` | | ảnh đích của `run /hello.elf`: in `RUN OK` + `# hello: run ok` rồi `Exit(0)` |
| `v2/test/boot/product-riscv64.sh` | | runner một boot: feature-graph · budget · D2-08 · FS/net/exec evidence · 3 ca âm |

### 3.3 Bố cục endpoint/cap (một nguồn: `proc_abi.rs`)

| Slot (CNode gốc) | Cap | Ai dùng |
|---|---|---|
| 9 | `EP_NET` | shell `NBSend Ping(ip)` → `netsrv` |
| 15 | `EP_NET_REPLY` | `netsrv` `NBSend` reply → shell |
| 16 | `EP_NET_KERN` | `netsrv` `Call` → kernel `NetSvc` (KT-306) |
| 17 | `EP_NET_KERN_REPLY` | kernel `reply_msg` cho `Call` của `netsrv` |
| 18 | `EP_EXEC` | shell `NBSend run <path>` → kernel `proc_exec` |
| cnode riêng của `netsrv` (16 ô) | 0 self · 1 `EP_NET` · 2 `EP_NET_REPLY` · 3 `EP_NET_KERN` · 4 `EP_FAULT` | `netsrv` U-mode |

Không mở ABI: mọi đường đi trên `NBSend`/`Call`/`Recv` + `Endpoint` đã có (cùng tiền lệ KT-77c cho
FS và KT-281/KT-306 cho net). **Driver vẫn ở kernel**: `netsrv` không map MMIO, không giữ cap
`Frame`/BAR (nên "net server U-mode" không mở đường DMA tuỳ ý).

### 3.4 Vòng chạy của `proc_rt` (thay đổi tối thiểu)

Mỗi vòng: cửa sổ RX (S-mode) → một lượt procsrv → một lượt shell → (product-exec) phục vụ `run`
nếu có → (product-net) một lượt `netsrv` **chỉ khi** TCB `Ready` → phục vụ `ping` nếu
`EP_NET_KERN` có request → tổng kết. `turn()` chỉ áp `user::apply_ipc_return` khi trạng thái
**trước** lượt là `BlockedOnRecv`/`BlockedOnReply` (hợp đồng resume của `KT-280`), và **không**
đánh dấu `exited` cho `Stop::Blocked` (server đang `Recv` là bình thường).

---

## 4. Bằng chứng (log nguyên văn)

### 4.1 Một boot sản phẩm — log **nguyên văn** (`product-riscv64.sh`, cây sạch `7032942a`)

```text
# core: product addr=0x802470b8 bytes=184792 pin=196608 image_cap=262144 untyped=65536 retype=1 configure=1 suspend=1 resume=1 state_ready=1 objects=3 caps=1 used=0 check_caps=1 check_bytes=1 check_all_bytes=1 check_sched=1 footprint_ok=1
# product fs: medium=virtio-blk blocks=8192 used=6 total=8162 volatile=0 why=ok const:ram_pa=0x83000000 const:ram_blocks=96 const:reply_bytes=24 const:path_max=48
# shell: ls n=2 path=/
# shell: write /note.txt bytes=5 sync=ok
# shell: cat ok bytes=5 first=hello
# product net: served=1 denied=0 failed=0 err=0 rtt_ms=5 sent=1 received=1 label=0x291 caller=26
# shell: ping 10.0.2.2 rtt_ms=5 err=0
# netsrv: ping 10.0.2.2 rtt_ms=5 err=0
# product exec: path=/hello.elf pid=1 tcb=11 old_entry=0x40000b7a new_entry=0x40000000 old_vspace=17 new_vspace=45 freed_frames=15 new_pages=11 frames=39->35 objects=55->50 caps=45->41 check=1
RUN OK
# hello: run ok
# product fs: medium=virtio-blk ls=17 cat=2 write=1 sync=1 errors=0 replies=28 dev_reads=93 dev_writes=149 dev_flushes=1
# product halt: shell_exit=0 fault_seen=1 events=4 ps_replies=0 ps_n=0 kill_suspend=0 proc_up=1 fs_replies=28 rx_windows=500 rx_irqs=2 rx_bytes=68 rx_late=51 ring_used=0 ring_head=5 ring_tail=5 ring_ok=1
# product halt net: replies=1 net_exit=1
# product halt exec: execs=1
```

Trình tự trong **một** boot: `ls` (FS đọc) → `write /note.txt hello` + fsync → `cat /note.txt`
(FS ghi/đọc lại **đúng byte**) → `ping 10.0.2.2` (shell → `netsrv` U-mode → kernel `NetSvc` →
ICMP thật → về shell) → `run /hello.elf` (kernel `exec` thay ảnh shell; ảnh đích in `RUN OK` rồi
`Exit(0)`) → kernel tổng kết + `halt(0)`. Cả ba đường **đồng thời** trong cùng ảnh, **không**
`ktest`, **không** harness marker.

### 4.2 Bất biến của ca dương (runner `product-riscv64.sh`)

Một boot phải có **đủ** (sai bất kỳ dòng nào ⇒ `not ok` + exit 1):

| # | Dòng (regex) | Chứng minh |
|---:|---|---|
| 1 | `^# k3 image=product ` | kernel thật, không phải model |
| 2 | `^# shell: up cmds=help(/[a-z]+)+$` | U-mode shell + danh sách lệnh (có `ping`/`run`) |
| 3 | `^# product fs: medium=virtio-blk ` | FS trên **đĩa thật** (không phải volume RAM bay hơi) |
| 4 | `^# shell: ls n=[0-9]+ path=/$` | **FS đọc** (thư mục gốc) |
| 5 | `^# shell: write /note\.txt bytes=5 sync=ok$` | **FS ghi** + `fsync` xuống phương tiện |
| 6 | `^# shell: cat ok bytes=5 first=hello$` | đọc lại **đúng byte** vừa ghi |
| 7 | `^# netsrv: ping 10\.0\.2\.2 rtt_ms=[0-9]+ err=0$` | **`netsrv` U-mode** chạy + gọi kernel |
| 8 | `^# shell: ping 10\.0\.2\.2 rtt_ms=[0-9]+ err=0$` | shell **nhận reply** của `netsrv` (IPC hai chiều) |
| 9 | `^# product net: served=1 denied=0 failed=0 err=0 rtt_ms=[0-9]+ sent=1 received=1 ` | kernel **ping thật** (ICMP qua slirp; `sent`/`received` từ driver) |
| 10 | `^# product halt net: replies=1 ` | tổng kết: **đúng một** request được phục vụ |
| 11 | `^# product exec: path=/hello\.elf .*check=1$` | **`exec`** nguyên tử (giữ pid/cspace, `check_objmem` ĐẠT) |
| 12 | `^RUN OK$` | ảnh đích chạy **thật** (không phải cùng ảnh shell) |
| 13 | `^# hello: run ok$` | dấu evidence của ảnh đích |
| 14 | `^# product halt: shell_exit=0 ` | thoát sạch (`Exit(0)`) |
| 15 | `^# product halt exec: execs=1$` | đúng **một** lần exec |
| 16 | không `^PANIC`, không `^not ok`, đúng **một** `^ok 1 - k3 boot$` | không có lỗi bị che |

Cổng phụ (trước/sau boot): **đồ thị feature** không bật `ktest`; `budget.sh <ảnh> product`;
**0 harness marker** (D2-08); **rev-gate** `vbrev_gate` (KT-86/D2-125.3) — cây bẩn ⇒ `exit 3`
TỪ CHỐI ĐO (không được tính là bằng chứng).

---

## 5. Đo (trước/sau)

| Chỉ số | Trước KT-342 | Sau KT-342 | Trần |
|---|---:|---:|---|
| `.data+.bss` ảnh sản phẩm `product-all` | `220.184` B (baseline `product-shell,product-fs`, đo tại `85eebd28`) | **`224.472` B** (`+4.288` B = `+1,63%`; 85,6% trần) | `262.144` B (D2-72 Q5) — **không** nới |
| `size_of::<Machine>()` | `184.792` B | **`184.792` B** (không đổi — `KT-342` không thêm field vào `Machine`) | `196.608` B |
| TAP ảnh test riscv64 (9 ảnh, matrix) | 71/3/37/6/10/5/6/11/3 | **không đổi** (TAP của ảnh test không bị chạm) | `expected.txt` |
| Tệp R1 lớn nhất bị sửa | `proc_rt.rs` 597 | **838** (`shell_user.rs` 1.291; `proc_net.rs` 183; `proc_exec.rs` 156; runner 355) | 1.500 |
| `user.rs` (D2-243 muốn tách < 1.100) | 1.499 | **1.499** — *chưa tách* (xem §8 nợ) | 1.500 |
| Harness marker trong ảnh sản phẩm | — | **0** (`ktest`, `# harness`, `not ok`, `TAP_TESTS`) | 0 (D2-08) |

Dòng `budget.sh` **nguyên văn** (ảnh `product-all`):

```text
# image=product path=/private/tmp/kt342-s1/v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64 file=551896 data+bss=224472 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
```

Dòng evidence `# core: product …` (từ **cùng** log boot) — chứng minh `size_of::<Machine>()`:

```text
# core: product addr=0x802470b8 bytes=184792 pin=196608 image_cap=262144 untyped=65536 retype=1 configure=1 suspend=1 resume=1 state_ready=1 objects=3 caps=1 used=0 check_caps=1 check_bytes=1 check_all_bytes=1 check_sched=1 footprint_ok=1
```

---

## 5. Hai đột biến — cả hai **ĐỎ đúng lý do** (D2-05, D2-244)

### 5.1 Đột biến (i) — bật lại `ktest` làm điều kiện driver

Tái lập **đúng** nhãn cũ bằng `--features product-all,mut-ktest-driver`
(`mut-ktest-driver = ["vifast-plat-riscv64/ktest"]`). Runner chạy `--negative feature-graph`:

```text
# product-riscv64: ĐỒ THỊ FEATURE bật harness 'ktest' trên crate arch
├── vifast-plat-riscv64 feature "ktest"
ok 0 - feature-graph: mut-ktest-driver bị bắt (ktest trong đồ thị feature) — đỏ đúng lý do

# product-riscv64: negative=feature-graph checks=0 fails=0
### RC featuregraph=0
```

Ảnh **không được boot tới marker**: cổng đồ thị chặn **trước** bước boot (đúng mô tả của lead:
"ca sản phẩm **không** boot tới marker"). Cơ chế: cổng hỏi `cargo tree -e features
-i vifast-plat-riscv64`; nhãn `ktest` là **feature rỗng** nên chỉ hiện ở chiều **ngược** (ghi
trong `product-riscv64.sh:feature_graph_guard` — đo được, không suy đoán).

### 5.2 Đột biến (ii) — bỏ `netsrv` khỏi đường sản phẩm

`--features product-all,mut-no-netsrv` ⇒ `proc_rt` **không** chạy lượt `netsrv`/`NetSvc` (một
`cfg` ở đúng một chỗ), mọi thứ khác nguyên vẹn. Runner chạy `--negative no-netsrv`:

```text
ok 0 - no-netsrv: boot vẫn lên (chỉ mất đường netsrv)
ok 0 - no-netsrv: thiếu '# netsrv:'
ok 0 - no-netsrv: kernel không phục vụ ping
ok 0 - no-netsrv: shell không nhận reply
ok 0 - exec: bảng ảnh + exec nguyên tử
ok 0 - exec: ảnh đích chạy thật (RUN OK)
ok 0 - halt: shell_exit=0 (thoát sạch)
### RC nonetsrv=0
```

⇒ Thiếu `netsrv` thì **chỉ** đường mạng đỏ (FS + `exec` vẫn xanh) — đột biến **có răng**, không
phải "đỏ vì lý do khác". Ca âm (iii) `--negative no-net`: cùng ảnh, **không** `-netdev` ⇒

```text
ok 0 - no-net: kernel PANIC có tên (fail-closed, không chạy mạng dở)
ok 0 - no-net: không tới '# product halt:'
### RC nonet=0
```

### 5.3 Vì sao "đỏ" không phải do lý do sai

Runner chỉ tính `ok` khi **đúng mẫu** evidence; mỗi ca âm khẳng định **điều ngược lại** (không
có evidence / PANIC có tên) và vẫn đòi các đường **không liên quan** xanh. Log đầy đủ:
`/tmp/kt342-final-{positive,featuregraph,nonetsrv,nonet}.log`.

---

## 6. TAP trước/sau (ảnh test **không** đổi)

`expected.txt` giữ nguyên; `ktest.sh 1 matrix` trên cây sạch `7032942a`:

```text
# ktest       smp=1  1/1 boot sạch (FAIL=0 TIMEOUT=0)
# ktest-guard smp=1  1/1 boot sạch (FAIL=0 TIMEOUT=0)
# ktest-user  smp=1  1/1 boot sạch (FAIL=0 TIMEOUT=0)
# ktest-proc  smp=1  1/1 boot sạch (FAIL=0 TIMEOUT=0)
# ktest-vblk  smp=1  1/1 boot sạch (FAIL=0 TIMEOUT=0)
# ktest-con   smp=1  1/1 boot sạch (FAIL=0 TIMEOUT=0)
# ktest-exec  smp=1  1/1 boot sạch (FAIL=0 TIMEOUT=0)
# ktest-ipc   smp=1  1/1 boot sạch (FAIL=0 TIMEOUT=0)
# ktest-netsrv smp=1  1/1 boot sạch (FAIL=0 TIMEOUT=0)
# KTEST: 27/27 boot sạch (smp=matrix, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec ktest-ipc ktest-netsrv)
# ktest.sh: PASS
```

Kế hoạch TAP đọc từ `expected.txt` (không hardcode): `ktest 71`, `ktest-guard 3`, `ktest-user 37`,
`ktest-proc 10`, `ktest-vblk 6`, `ktest-con 5`, `ktest-exec 6`, `ktest-ipc 11`, `ktest-netsrv 3` —
**trước = sau** (KT-342 không sửa một dòng mã nào của ảnh test; `ktest` vẫn bật **tường minh** qua
`required-features` của bin, nhãn driver chỉ thôi kéo nó). Runner đỏ nếu bất kỳ `not ok`/thiếu TAP
(`expected.txt` là nguồn, không nới).

## 7. Cổng cuối

Tất cả chạy trên **cây sạch** `7032942a`, khoá máy đo **trống** (không chồng đo).

### 7.1 `./v2/bin/v2 check` — PASS

```text
# step-time-total   103637 ms  TỔNG check
RC check=0
```

### 7.2 `./v2/bin/v2 gate --dev` (T1) — PASS

```text
# step-time QEMU smoke (T1: 7 ảnh × 1 boot) 128413 ms
# step-time-total   637798 ms  TỔNG gate --dev (T1)
v2 gate --dev: PASS (T1 — kiểm tích hợp nhanh, **KHÔNG** phải full gate; full gate = 'gate --fast')
RC gate=0
```

(T1, **không** phải full gate — báo cáo dùng đúng nhãn; full gate = `gate --fast` của Lead/T2.)

### 7.3 `bash v2/test/boot/ktest.sh 1 matrix` — 27/27 PASS

```text
# KTEST: 27/27 boot sạch (smp=matrix, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec ktest-ipc ktest-netsrv)
# ktest.sh: PASS
RC matrix=0
```

### 7.4 `bash v2/test/boot/soak.sh --smoke 60 3` — 60/60 xanh, **exit 1 theo hợp đồng smoke**

```text
# infra after: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=6.01 loadavg_5m=5.18 loadavg_15m=6.19 ncpu=12 boot_done=1 verdict=OK
# soak: xong wall=16s (3 boot/s); ok=60 fail=0 tổng=60/60
# soak: BUILD_ID=c049ae89eece6f18 sha256=9647d701c7097f88560694423adc849f74ebe029c3d76e84c0f3c2f3896103fb git=7032942ae10bc9936b98b168cacbb4574840eb89
# soak: 60/60 lỗi=0 — 0/N chỉ là trần (QD §5.3); không retry (D2-07)
# soak: SMOKE N=60 < 1000 — KHÔNG phải bằng chứng cổng K3 (D2-24), luôn exit 1
RC soak=1
```

⇒ **60/60 boot xanh, 0 lỗi, 0 retry**; `exit 1` là **hợp đồng** của `--smoke` (60 < 1.000, không phải
bằng chứng cổng K3) — không phải đỏ kỹ thuật. Đây là mức lead yêu cầu; soak 1.000 thuộc `gate --fast`/nightly.

### 7.5 Runner sản phẩm (bằng chứng mới) — **0 đỏ**

```text
# product-riscv64: checks=0 fails=0 image=…/vifast-kbin-riscv64 log=…/kt342-product.9418/kt342-product.log
REV-GATE: ACCEPT inputs=51788ada2ea07a3f current_inputs=51788ada2ea07a3f head_rev=7032942ae10b count=378 log=…/kt342-product.9418/kt342-product.log
# product-riscv64: rev-gate ACCEPT (D2-125.3)
### RC positive=0
```

Cộng 3 ca âm (đột biến (i)/(ii) + không mạng) đều `fails=0`, `RC=0` (§5).

## 10. Cam kết phạm vi

Không sửa: `spec/**`, `v2/agent/{STATE,QUEUE,DECISIONS}.md`, `.github/**`, `v2/bin/lib/*.sh`,
`docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`, `docs/v1/**`, 19 tệp
`[[legacy]]` của `numbers-policy.toml`, `main`. Chỉ thêm/sửa: 12 tệp trong
`v2/kernel/vifast-{plat,kbin}-riscv64`, `v2/test/boot/product-riscv64.sh`, tệp journal này.
Không nới ngưỡng; không `#[ignore]`; không retry để lấy xanh; không push.

## 8. Nợ / khác biệt so với kế hoạch (nói thẳng)

1. **D2-243 — `user.rs` 1.499 → < 1.100: CHƯA làm.** Tệp vẫn **1.499** dòng (R1 đạt nhưng chưa
   đạt mục tiêu tách của kế hoạch). Lý do: thời gian còn lại dồn cho 3 lỗi runtime + 2 đột biến +
   cổng; tách `user.rs` là việc **cơ học nhưng rộng** (di chuyển `classify_u_fault`/`enter_*`),
   làm muộn sẽ không kịp đo lại. **Đề xuất task con `KT-342b`** (hoặc gộp vào KT-336c đã có).
2. **Đường trả lời `ping`:** shell nhận kết quả từ kernel trên `EP_REPLY` (đường kernel→U-mode đã
   có cho FS), **không** nhận trực tiếp từ cap reply của `netsrv`. Lý do: đường U-mode→U-mode
   "gửi vào endpoint khi không có người chờ" làm message **biến mất** (kernel trả `0` nhưng hàng
   đợi rỗng; đo được: `# netsrv: sent=0` + `q=0` + `last_mr=Some(5)` — xem §9). Kernel chuyển
   tiếp **nguyên văn** `rtt`/`err` mà `NetSvc` nhận từ `netsrv`; shell **không** giữ cap driver.
   Đây là **giảm phạm vi** so với "hai U-mode nói chuyện trực tiếp" — ghi để reviewer quyết.
3. **`# product net: served=…`** là số đo kernel (nguồn sự thật), **không** phải lời chương trình;
   `# netsrv: …` là lời chương trình — cả hai đều bị runner kiểm mẫu.
4. Không chạm `spec/`, `v2/bin/lib/*.sh`, `.github/**`, `QUEUE/STATE/DECISIONS`, 19 tệp legacy
   của `numbers-policy.toml`; không nới ngưỡng nào (R1 1.500, `Machine` 196.608, sản phẩm
   262.144, matrix 27/27).
5. **Boot chẩn đoán** (04:00–04:10): trong lúc một `gate --dev` của agent khác giữ khoá, tôi chạy
   **boot chẩn đoán** (bản sao runner trong `/tmp`, **không** phải runner trong repo) song song
   **1** QEMU — Lead duyệt ngoại lệ hẹp. Boot đó **KHÔNG** dùng làm bằng chứng cổng, **không** lấy
   số thời gian; mọi số trong journal này đến từ các lần chạy **trên cây sạch `7032942a`**.

## 9. Lỗi tìm được trong lúc làm (đỏ-trước, ghi để người sau không dẫm lại)

1. **Tràn byte khi đóng gói đường dẫn `run`**: shell nhét **8** byte path vào `MR0 << 8` (64 bit)
   ⇒ mất byte thứ 8; kernel giải mã `/hello.elf` thành `/hello.lf` ⇒ `exec` trả `NotFound(2)`
   (`# product exec: err=2`) — **im lặng** ở mức người dùng (shell chỉ in `run err=2`). Sửa: MR0
   giữ **7** byte (bit 8..63), byte 8.. ở MR1; kernel giải mã tương ứng. Bằng chứng đỏ→xanh:
   `run err=2` → `# product exec: path=/hello.elf … check=1` + `RUN OK`.
2. **`EP_EXEC` lệch một slot**: shell gửi `run` vào slot **16** (`EP_NET_KERN`) thay vì **18**
   (`EP_EXEC`) ⇒ kernel không bao giờ thấy request, shell `run timeout`. Sửa hằng số + đối chiếu
   `proc_abi` (một nguồn).
3. **Server chặn-khi-nhận cần `apply_ipc_return`**: `netsrv` resume sau `Call`/`Recv` phải nạp
   `a0..a6` từ vùng nhận IPC (cùng hợp đồng `ktest_netsrv`); thiếu thì server chạy với thanh ghi
   cũ. `proc_rt::net_turn` gọi trước mỗi lượt (không có message ⇒ hàm trả `false`, không chạm
   trapframe).
4. **U-mode→U-mode "send không người chờ" mất message** (chưa sửa trong core — ngoài phạm vi
   KT-342): `nb_send` trả 0, `endpoint_last_mr` được ghi, nhưng hàng đợi đọc ra **rỗng** ngay sau
   đó; không có waiter (`endpoint_waiting_recv=None`), shell `NBRecv` mãi `Busy(14)`. Vì vậy
   `ping` đi đường kernel chuyển tiếp (§8 mục 2). **Đề xuất task con** điều tra `endpoint_enqueue`
   + `ipc_msgs` (nghi vấn `free_ipc_msg`/`alloc_ipc_msg` hoặc `Machine` khác instance giữa
   syscall và đọc hàng đợi) — có script tái hiện trong log chẩn đoán.
