# KT-251 — chốt K6: xác minh **S1** trên ảnh sản phẩm (2026-11-21)

Người ghi: sub-agent `netlib` · Nhánh `task/kt243-net` · Base `main` @ `43a054e3`
· HEAD khi đo: `8caae70a` (`[KT-244] crate vifast-net`) · Máy: macOS arm64 (ps), Rust 1.100.0
· QEMU 11.1.3, OpenSBI `fw_dynamic` (`/opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin`).

## 1. Vì sao phải đo lại (không suy đoán)

`v2/agent/reports/K6-DOD.md` §3.2 ghi **S1 = KHÔNG ĐẠT** với lý do: ảnh sản phẩm `halt` sau
`# k3 halt`, U-mode nằm sau feature `user`, shell chỉ có trong ảnh test `ktest-proc`. Kết luận đó
đo trên ảnh **trước** KT-77b (`product-shell`) và KT-77c (`product-fs`).

Hai task đó đã đưa shell U-mode + FS vào **chính** ảnh sản phẩm (feature `product-shell`,
`product-fs`). Vì vậy kết luận cũ **không còn** đúng với ảnh hiện tại — phải **đo lại**, không
được suy đoán theo mã nguồn. KT-251 làm đúng việc đó: build ảnh sản phẩm từ HEAD, boot QEMU, gửi
kịch bản shell qua serial, đọc log.

## 2. Ảnh đo — lệnh nguyên văn + danh tính

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd /private/tmp/kt243-s2
VIFAST_REPRO_CARGO_EXTRA="--features product-shell,product-fs" \
VIFAST_REPRO_LOG=/tmp/kt251-build.log \
  bash v2/tools/repro-build.sh --target-dir /tmp/kt251-target
```

Log nguyên văn: `v2/agent/JOURNAL/kt251-repro.log`. Dòng `REPRO` (nguồn của mọi số dưới đây):

```text
REPRO machine=Darwin-arm64 root=/private/tmp/kt243-s2 sha256=2eced41f8e82031dc56599a06047070640f6c211ac6aca199bba8985f958be74 size=213632 wall_s=25 algo=kt134-v4 inputs=e5992dbf68072503 dirty=0 rev=8caae70af67a build=f861bd283066120f text=131728 image=/private/tmp/kt251-target/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
```

| Khóa đầu vào (AGENTS §6.3) | Giá trị |
|---|---|
| commit | `8caae70af67abcc56bd663c4530bae21a47ec063` (HEAD, cây sạch) |
| build-inputs | `e5992dbf68072503` (`algo=kt134-v4`, `dirty=0`) |
| toolchain | `1.100.0-aarch64-apple-darwin` (rustc 1.100.0, cargo 1.100.0) |
| feature | `product-shell,product-fs` |
| sha256 ảnh | `2eced41f8e82031dc56599a06047070640f6c211ac6aca199bba8985f958be74` |
| cỡ ảnh | `213632` byte |
| dòng `# build:` trong ảnh | `algo=kt134-v4 inputs=e5992dbf68072503 dirty=0` (`kt251-s1-smp1.log:68`) |
| `build=` (danh tính nội dung) | `f861bd283066120f`, `text=131728` |

**Nợ P20**: sha256 phụ thuộc **thư mục build** (`/tmp/kt251-target`); danh tính bền là dòng
`# build: algo=… inputs=… dirty=…` + `build=…`, không phải sha. Cổng danh tính `vbrev_gate` đã
**ACCEPT** cả hai log (`inputs` khớp HEAD, `dirty=0`).

## 3. Boot + kịch bản serial

Script đo: `v2/agent/JOURNAL/kt251-s1-measure.sh` (dùng `qemu_guard.sh` KT-78 +
`stdin_wrap.sh` KT-77a — cùng khung với `kt192-closure-measure.sh` §F1, không tự viết lại vòng đời
QEMU). Payload gửi qua serial (UART → PLIC → ring → shell U-mode):

```text
help
ls /
cat /README
exit
```

QEMU: `-machine virt,aia=none,aclint=on -bios <opensbi fw_dynamic> -kernel <ảnh> -nographic
-cpu rv64 -m 128M -smp 1|2 -no-reboot`, timeout 30 s/boot.

| Lần | smp | qemu rc | giây | log serial |
|---|---:|---:|---:|---|
| 1 | 1 | 0 | 2 | `v2/agent/JOURNAL/kt251-s1-smp1.log` |
| 2 | 2 | 0 | 3 | `v2/agent/JOURNAL/kt251-s1-smp2.log` |

Log tổng hợp lần đo: `v2/agent/JOURNAL/kt251-s1-measure.log`; log guard:
`v2/agent/JOURNAL/kt251-qemu-guard.log` (`isolated=1`, `rc=0`).

## 4. Bằng chứng — từng dòng, cả hai smp

| Phép kiểm | smp 1 (`kt251-s1-smp1.log`) | smp 2 (`kt251-s1-smp2.log`) |
|---|---|---|
| dòng `# build:` của ảnh | `:68` | `:68` |
| `# product shell: harts=…` (shell U-mode có mặt) | `:85` | `:85` |
| `# shell: up cmds=…` (17 lệnh, có `ls`/`cat`/`cd`/`pwd`) | `:89` | `:89` |
| prompt `shell>` | `:91`, `:94`, `:98`, `:103` | `:91`, `:94`, `:98`, `:103` |
| `# shell: help cmds=17` | `:92` | `:92` |
| `# shell: ls n=2 path=/` (FS chạy được) | `:97` | `:97` |
| `# shell: cat ok bytes=79 first=ViFastOs` (đọc tệp đã seed) | `:102` | `:102` |
| `# shell: exit code=0` | `:104` | `:104` |
| `# product halt: shell_exit=0 …` (thoát sạch) | `:109` | `:109` |

Trích nguyên văn (smp 1):

```text
# build: algo=kt134-v4 inputs=e5992dbf68072503 dirty=0
# product shell: harts=1 procsrv=0xd shell=0xf victim=0x11 kern=0x13 const:ep_proc=2 const:ep_ps=5 const:ep_events=8 const:slot_cspace=20 const:input_mode=serial const:inject=0
# shell: up cmds=help/echo/ps/wait/kill/ls/cd/pwd/cat/write/time/uptime/sync/stat/truncate/jobs/exit
shell>
# shell: help cmds=17
  help | echo <text> | ps | wait <pid> | kill <pid> | ls [path] | cd [path] | pwd | cat <path> | write <path> <text> | time | uptime | sync | stat <path> | truncate <path> <size> | jobs | exit
shell>
  f README
  d docs
# shell: ls n=2 path=/
shell>
ViFastOs v2 - FS tren anh san pham
lenh: ls | cat <path> | write <path> <text>

# shell: cat ok bytes=79 first=ViFastOs
shell>
# shell: exit code=0
# sys: nr=14 err=0 value=0 tcb=15 state=BlockedOnSend src=core
# u-mode exit: code=0 faultkind=Exit{0} tcb=15 state=BlockedOnSend
# proc: recv exit pid=15 code=0
# product fs: medium=ram ls=17 cat=5 write=0 sync=0 errors=0 replies=26 dev_reads=0 dev_writes=0 dev_flushes=0
# product halt: shell_exit=0 fault_seen=1 events=4 ps_replies=0 ps_n=0 kill_suspend=0 proc_up=1 fs_replies=26 rx_windows=27 rx_irqs=0 rx_bytes=54 rx_late=54 ring_used=2 ring_head=4 ring_tail=2 ring_ok=1
```

Trích nguyên văn (smp 2, phần khác biệt):

```text
# product shell: harts=2 procsrv=0xd shell=0xf victim=0x11 kern=0x13 const:ep_proc=2 const:ep_ps=5 const:ep_events=8 const:slot_cspace=20 const:input_mode=serial const:inject=0
# shell: help cmds=17
# shell: ls n=2 path=/
# shell: cat ok bytes=79 first=ViFastOs
# shell: exit code=0
# product halt: shell_exit=0 fault_seen=1 events=4 ps_replies=0 ps_n=0 kill_suspend=0 proc_up=1 fs_replies=26 rx_windows=27 rx_irqs=0 rx_bytes=108 rx_late=108 ring_used=5 ring_head=7 ring_tail=2 ring_ok=1
```

## 5. Kết luận S1

Tiêu chí S1 (nguyên văn, **không sửa**): *"Ảnh SẢN PHẨM có U-mode/shell tương tác như v1"*.

Điều kiện KT-251 đặt ra (đo được, không suy đoán): log serial của **chính ảnh sản phẩm** phải có
`shell>` **và** `help cmds=<N>` **và** ít nhất một lệnh FS chạy được **và** `exit` sạch.

| Điều kiện | smp 1 | smp 2 |
|---|---|---|
| `shell>` | có (`:91`) | có (`:91`) |
| `help cmds=17` | có (`:92`) | có (`:92`) |
| lệnh FS chạy được | `ls n=2 path=/` (`:97`), `cat ok bytes=79` (`:102`) | như smp 1 |
| `exit` sạch | `exit code=0` (`:104`), `product halt: shell_exit=0` (`:109`) | như smp 1 |

⇒ **S1 = ĐẠT** trên ảnh `2eced41f…` (dòng `# build: algo=kt134-v4 inputs=e5992dbf68072503
dirty=0`), cả smp 1 và smp 2.

**Ghi chú trung thực (không giấu)**:

- Ảnh này boot **không** có đĩa PCI ⇒ `# product fs: medium=ram … why=khong-co-pci` (`:87`): FS là
  volume RAM 96 block, **không** phải virtio-blk. S1 chỉ đòi "một lệnh FS chạy được"; đường
  virtio-blk trên ảnh sản phẩm là bằng chứng khác (`ktest-vblk`, KT-72) và **không** thuộc S1.
- `# product shell: … const:input_mode=serial` (`:85`): bàn phím là serial (đúng như v1 dùng
  `--input-check` trên serial), không phải virtio-keyboard.
- Kết luận này **chỉ** nói về S1. C1..C8 và N1..N7 của `K6-DOD.md` **không** bị đụng; không tiêu
  chí nào bị sửa, không ngưỡng nào bị nới.

## 6. Đã sửa gì

- `v2/agent/reports/K6-DOD.md`: thêm mục **"Cập nhật 2026-11-21 (KT-251)"** ở đầu tệp + sửa **duy
  nhất** dòng **S1** ở §3.2 (giữ nguyên văn tiêu chí; thêm `log:dòng`; ghi sha ảnh + `# build:`).
  Không sửa C1..C8, N1..N7, không sửa tiêu chí, không đổi ngưỡng.
- Thêm log bằng chứng vào `v2/agent/JOURNAL/` (liệt kê §7).

## 7. Tệp

| Tệp | Nội dung |
|---|---|
| `v2/agent/JOURNAL/kt251-repro.log` | log `repro-build.sh` (dòng `REPRO` = nguồn sha/inputs/build) |
| `v2/agent/JOURNAL/kt251-s1-measure.sh` | script đo (boot + bơm serial + đếm dòng bằng chứng) |
| `v2/agent/JOURNAL/kt251-s1-measure.log` | log tổng hợp của script đo |
| `v2/agent/JOURNAL/kt251-s1-smp1.log` | log serial nguyên văn, smp 1 (109 dòng) |
| `v2/agent/JOURNAL/kt251-s1-smp2.log` | log serial nguyên văn, smp 2 (109 dòng) |
| `v2/agent/JOURNAL/kt251-qemu-guard.log` | log `qemu_guard.sh` (process group, `rc=0`) |
| `v2/agent/JOURNAL/kt251-k6-closure.md` | tệp này |

---

## Phụ lục — đo lại **S1** trên **HEAD cuối tuần** (`6309b6a6`, 2026-11-21 02:41 JST)

Lý do đo lại: sau KT-251, `main` nhận thêm 8 commit (KT-244/245/246/246b/247/248/249/250/252) ⇒
`build-inputs` đổi ⇒ bằng chứng S1 phải gắn với **đầu vào mới** (AGENTS §6.3), không ghép ảnh cũ.

```text
$ VIFAST_REPRO_CARGO_EXTRA="--features product-fs" bash v2/tools/repro-build.sh
REPRO … sha256=e78f58d6d99c140aa6dccae283c3c921d0652881bd5f8259a61455e9421fcf1f size=213632
       algo=kt134-v4 inputs=f4934dc81f0c903e dirty=0 rev=6309b6a66b5b build=f861bd283066120f text=131728
$ KT251_LOGDIR=/tmp/kt251b KT251_PREFIX=kt251b KT251_IMAGE=<ảnh trên> bash JOURNAL/kt251-s1-measure.sh
```

| Phép đo | smp 1 | smp 2 |
|---|---|---|
| boot `qemu_rc` | 0 (2 s) | 0 (2 s) |
| `shell>` | 4 dòng | 4 dòng |
| `help cmds=` | **17** | **17** |
| `ls n=2 path=/` | 1 | 1 |
| `cat ok` | 1 | 1 |
| `exit code=0` | 1 | 1 |
| `REV-GATE` | **ACCEPT** (`inputs=f4934dc8…`, `head_rev=6309b6a6…`, `count=257`) | **ACCEPT** |

Log: [`kt251b-s1-smp1.log`](kt251b-s1-smp1.log), [`kt251b-s1-smp2.log`](kt251b-s1-smp2.log).
**Kết luận**: S1 **ĐẠT** giữ nguyên trên HEAD cuối tuần; `build=f861bd283066120f` và `text=131728`
**trùng** bản đo trước ⇒ mã sản phẩm không đổi, chỉ dòng `# build:` (đầu vào) đổi.

---

## Phụ lục 2 — đo lại S1 trên HEAD **sau vòng 2** (`e88f5f77`, 2026-11-21 04:35 JST)

`build-inputs` đổi lần nữa (thêm `vifast-kbin-x86_64`, `virtio-input`, `wake_scope.py`) ⇒ đo lại.

```text
REPRO … sha256=94fc1c6e09e9dd569666c95ebb2bc36ac639027afd52155e764d1d3855048664 size=213632
       algo=kt134-v4 inputs=c981995a5d55b107 dirty=0 rev=e88f5f77a45d build=f861bd283066120f text=131728
```

| Phép đo | smp 1 | smp 2 |
|---|---|---|
| `shell>` | 4 dòng | 4 dòng |
| `help cmds=` | **17** | **17** |
| `ls n=2 path=/` | 1 | 1 |
| `cat ok` | 1 | 1 |
| `exit code=0` | 1 | 1 |
| `REV-GATE` | **ACCEPT** (`inputs=c981995a…`, `count=270`) | **ACCEPT** |

**`build=f861bd283066120f` + `text=131728` trùng** cả ba lần đo (trước cuối tuần, sau vòng 1, sau
vòng 2) ⇒ mã sản phẩm **không đổi**; chỉ dòng `# build:` (đầu vào) đổi.

### Flake tìm thấy và đã sửa (T-INFRA)

Lần đo đầu trên HEAD này: **smp 2** in `# shell: ?` cho `help` (1/3 lần). Nguyên nhân: script bơm
payload với `KT77_SEND_DELAY=0.05` — shell U-mode của ảnh sản phẩm khởi động chậm hơn ảnh test, nên
dòng `help` đầu tiên tới **trước** khi shell sẵn sàng. **Không** phải lỗi sản phẩm: `ls`/`cat`/`exit`
xanh 3/3 lần. Đã sửa: `KT251_SEND_DELAY` (mặc định **0,5 s**) — chạy lại **3 lần × smp 1,2**:
**6/6 đạt đủ 5 phép kiểm**. Đây là **tham số thời điểm bơm**, không phải ngưỡng đạt.

---

## Phụ lục 3 — đo lại S1 trên HEAD **sau vòng 4** (`a52a1e3e`, 2026-11-21 06:55 JST)

`build-inputs` đổi lần nữa (thêm `virtio-gpu`, `dns`, `smp`/`gic`/`shootdown`, `idt`/`tss` x86_64).

```text
REPRO … sha256=6f492f4967e412c142125d06df6ebef1efe797349311fab1645735f37177974f size=213632
       algo=kt134-v4 inputs=0fa34f818a29c399 dirty=0 rev=a52a1e3ebe1e build=f861bd283066120f text=131728
```

| Phép đo | smp 1 | smp 2 |
|---|---|---|
| `shell>` | 4 dòng | 4 dòng |
| `help cmds=` | **17** | **17** |
| `ls n=2 path=/` | 1 | 1 |
| `cat ok` | 1 | 1 |
| `exit code=0` | 1 | 1 |
| `REV-GATE` | **ACCEPT** (`inputs=0fa34f81…`, `count=283`) | **ACCEPT** |

**`build=f861bd283066120f` + `text=131728` trùng cả bốn lần đo** (trước cuối tuần, sau vòng 1, 2, 4)
⇒ mã sản phẩm **không đổi** qua 21 task; chỉ dòng `# build:` (đầu vào) đổi. Log:
[`kt251d-s1-smp1.log`](kt251d-s1-smp1.log), [`kt251d-s1-smp2.log`](kt251d-s1-smp2.log).

---

## Phụ lục 4 — đo lại S1 trên HEAD **sau vòng 7** (`6dc3a2bb`, 2026-11-21 10:15 JST)

```text
REPRO … sha256=93f04f0614543831166f3f95d735a50c16ffb941634dd1aa46eddb17997c1be5 size=213632
       algo=kt134-v4 inputs=6bdf06812e61a889 dirty=0 rev=6dc3a2bbe336 build=f861bd283066120f text=131728
```

| Phép đo | smp 1 | smp 2 |
|---|---|---|
| `shell>` | 4 dòng | 4 dòng |
| `help cmds=` | **17** | **17** |
| `ls n=2 path=/` | 1 | 1 |
| `cat ok` | 1 | 1 |
| `exit code=0` | 1 | 1 |
| `REV-GATE` | **ACCEPT** (`inputs=6bdf0681…`, `count=292`) | **ACCEPT** |

**`build=f861bd283066120f` + `text=131728` trùng cả năm lần đo** (trước cuối tuần, sau vòng 1, 2, 4,
7) ⇒ mã sản phẩm **không đổi** qua **30 task**; chỉ dòng `# build:` (đầu vào) đổi. Log:
[`kt251f-s1-smp1.log`](kt251f-s1-smp1.log), [`kt251f-s1-smp2.log`](kt251f-s1-smp2.log).

**Ghi chú kỹ thuật**: lần đo đầu trên HEAD này **bị `REV-GATE` TỪ CHỐI** (`inputs-mismatch`) vì tôi
chọn nhầm ảnh trong thư mục tạm cũ (`ls -d … | tail -1`). Đây là **cổng làm đúng việc của nó** —
không ghép bằng chứng từ ảnh khác đầu vào. Đo lại với **đúng** đường dẫn ảnh do `repro-build.sh`
in ra ⇒ `ACCEPT`.
