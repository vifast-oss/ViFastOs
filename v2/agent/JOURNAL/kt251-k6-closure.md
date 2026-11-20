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
