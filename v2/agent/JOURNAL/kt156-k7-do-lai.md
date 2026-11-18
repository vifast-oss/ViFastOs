# KT-156 — ĐO LẠI SÁU TIÊU CHÍ K6 TRÊN ẢNH SẢN PHẨM CUỐI CÙNG

**Ngày**: 2026-11-18/19 (JST) · **Agent 2 (L9)** · nhánh `night/L9-k7measure` · worktree
`/private/tmp/night-L9-k7measure` · HEAD `ba7223da23b8` (tách từ `main` tại `ba7223da`) · cây **sạch**
(`dirty=0`, đo bằng dòng `# build:` của ảnh, xem §1).

**Việc được giao**: đo lại **sáu tiêu chí K6** (OD-K6-2) trên **ảnh sản phẩm cuối cùng** để làm bằng
chứng đóng **K7**. Đây là task **ĐO**: không sửa mã sản phẩm, không sửa `spec/`, không nới ngưỡng.

**Script đo**: [`kt156-closure-measure.sh`](kt156-closure-measure.sh) (chép từ
[`kt129-closure-measure.sh`](kt129-closure-measure.sh), giữ nguyên cấu trúc + phép kiểm) +
[`kt156-f3-negative.sh`](kt156-f3-negative.sh) (F3, xem §5 — lý do phải tách).

## 0. Kết luận — sáu tiêu chí

| Mã | Tiêu chí (OD-K6-2) | Kết quả | Bằng chứng (log + dòng nguyên văn) |
|---|---|---|---|
| **F1** | `shell>` xuất hiện ở **1, 2 và 4 hart** | **ĐẠT** | `kt156-f1.log` — §2.1 |
| **F2** | chạy hết kịch bản shell, **gồm đọc lại tệp sau reboot** | **ĐẠT** | `kt156-f2.log` — §2.2 |
| **F3** | **ca âm**: ảnh **tắt shell** thì kịch bản phải **đỏ** | **ĐẠT** | `kt156-f3.log` — §2.3 |
| **F4** | ảnh sản phẩm vào **ma trận** (8 ảnh × 3 hart = 24 boot) **và** soak **1.000** | **ĐẠT** (ma trận **21/21**, soak **1000/1000**) | `kt156-f4.log` — §2.4 |
| **F5** | **ngưỡng giữ nguyên** (kích thước ảnh / `data+bss` ≤ 262.144) | **ĐẠT** | `kt156-f5.log` — §2.5 |
| **F6** | ảnh sản phẩm **không chứa harness** (quét bằng máy) | **ĐẠT** | `kt156-f6.log` — §2.6 |

**Không có tiêu chí nào CHƯA ĐO.** Cả sáu đều có lệnh + dòng log nguyên văn trong
`v2/agent/JOURNAL/kt156-logs/`.

**Một sai lệch so với đề bài, đã đo được và sửa** (chi tiết §5): đề bài ghi build ca âm F3 bằng
`--features product-fs` (không `product-shell`), nhưng trong cây này `product-fs` **kéo theo**
`product-shell` ⇒ build ra **đúng cùng sha** ảnh sản phẩm. Ca âm đúng là build **không feature nào**.

## 1. Ảnh đo + dấu vân tay

**Ảnh sản phẩm** (build bằng pipeline canonical, ghim theo D2-122.6):

```text scope=kt156-logs/kt156-build-product.out
REPRO machine=Darwin-arm64 root=/private/tmp/night-L9-k7measure sha256=e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71 size=209536 wall_s=15 algo=kt134-v4 inputs=0b8016e6cb6038d5 dirty=0 rev=ba7223da23b8 build=b080e40c1bd815d1 text=130722 image=/private/var/folders/jp/05hgrzxj65bgh355l2zs2zz80000gn/T/vifast-repro-target.V4F7Yr/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
```

Lệnh build (nguyên văn, chạy trong worktree):

```bash
VIFAST_REPRO_CARGO_EXTRA="--features product-shell,product-fs" \
  VIFAST_REPRO_LOG=v2/agent/JOURNAL/kt156-logs/kt156-build-product.log \
  bash v2/tools/repro-build.sh
```

Ghim ảnh (D2-122.6) — mọi phép đo dưới đây trỏ vào **tệp đã ghim**, không trỏ `target/`:

```text scope=kt156-logs/kt156-pin.log
PIN sha=e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71 size=209536 path=/private/tmp/night-L9-k7measure/v2/artifacts/e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71.bin # build: algo=kt134-v4 inputs=0b8016e6cb6038d5 dirty=0 state=da-co
```

**Dấu vân tay build-inputs** (dòng `# build:` do kernel tự in, đọc từ log boot — không chép tay):

```text scope=kt156-logs/kt156-boot-smp1.log
# build: algo=kt134-v4 inputs=0b8016e6cb6038d5 dirty=0
```

```text scope=kt156-logs/kt156-f1.log
REV-GATE: ACCEPT inputs=0b8016e6cb6038d5 current_inputs=0b8016e6cb6038d5 head_rev=ba7223da23b8 count=213 log=/private/tmp/night-L9-k7measure/v2/agent/JOURNAL/kt156-logs/kt156-boot-smp1.log
```

`REV-STATE` (từ `persist.sh`, cùng cây) ghi rõ phạm vi dấu vân tay:

```text scope=kt156-logs/kt156-master.log
REV-STATE: root=/private/tmp/night-L9-k7measure head_rev=ba7223da23b8 inputs=0b8016e6cb6038d5 count=213 dirty=0 scope=[v2/kernel v2/lib v2/Cargo.toml v2/Cargo.lock v2/rust-toolchain.toml v2/tools/repro-build.sh v2/tools/rustc-canon.rs]
```

**Ảnh ca âm F3** (build lần hai, **không feature**): sha
`1287611cb38c42446c618acf8bba94aba82c50bed53a632656fcb4520ecffe0b`, 82.552 B — §2.3.

## 2. Từng tiêu chí — lệnh + dòng log nguyên văn

### 2.1 F1 — `shell>` ở 1, 2 và 4 hart

Lệnh (trong `kt156-closure-measure.sh`, mục `F1`): boot ảnh **đã ghim** ở `-smp 1/2/4`, bơm
`help\nexit\n` qua `stdin_wrap.sh`, đi qua `qemu_guard.sh` (D2-116):

```bash
KT77_SEND_DELAY=0.05 qg_boot "$L" 300 v2/test/parity/stdin_wrap.sh "$PAY" \
  "$(command -v qemu-system-riscv64)" \
  -machine virt,aia=none,aclint=on -bios /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
  -kernel v2/artifacts/e7149448….bin -nographic -cpu rv64 -m 128M -smp "$SMP" -no-reboot
```

```text scope=kt156-logs/kt156-f1.log
### boot smp=1 qemu_rc=0 secs=3 log=/private/tmp/night-L9-k7measure/v2/agent/JOURNAL/kt156-logs/kt156-boot-smp1.log
# build: algo=kt134-v4 inputs=0b8016e6cb6038d5 dirty=0
# product shell: harts=1 procsrv=0xd shell=0xf victim=0x11 kern=0x13 const:ep_proc=2 const
# shell: up cmds=help/echo/ps/wait/kill/ls/cd/pwd/cat/write/time/uptime/sync/stat/truncate/jobs/exit
# smp: boot=0 harts=1 present=0x1 online=0x1 started=0x0 absent=0xfe t
### smp=1 shell_prompt_lines=2
### boot smp=2 qemu_rc=0 secs=2 log=/private/tmp/night-L9-k7measure/v2/agent/JOURNAL/kt156-logs/kt156-boot-smp2.log
# build: algo=kt134-v4 inputs=0b8016e6cb6038d5 dirty=0
# product shell: harts=2 procsrv=0xd shell=0xf victim=0x11 kern=0x13 const:ep_proc=2 const
# shell: up cmds=help/echo/ps/wait/kill/ls/cd/pwd/cat/write/time/uptime/sync/stat/truncate/jobs/exit
# smp: boot=0 harts=2 present=0x3 online=0x3 started=0x2 absent=0xfc t
### smp=2 shell_prompt_lines=2
### boot smp=4 qemu_rc=0 secs=2 log=/private/tmp/night-L9-k7measure/v2/agent/JOURNAL/kt156-logs/kt156-boot-smp4.log
# build: algo=kt134-v4 inputs=0b8016e6cb6038d5 dirty=0
# product shell: harts=4 procsrv=0xd shell=0xf victim=0x11 kern=0x13 const:ep_proc=2 const
# shell: up cmds=help/echo/ps/wait/kill/ls/cd/pwd/cat/write/time/uptime/sync/stat/truncate/jobs/exit
# smp: boot=0 harts=4 present=0xf online=0xf started=0xe absent=0xf0 t
### smp=4 shell_prompt_lines=2
```

Dòng `# smp: boot=` đầy đủ (không cắt) trong từng log boot:

```text scope=kt156-logs/kt156-boot-smp1.log
# smp: boot=0 harts=1 present=0x1 online=0x1 started=0x0 absent=0xfe timeout=0 dtb=0x87e00000 dtb_magic=1 base=0x80400000 stride=0x1d000
```

```text scope=kt156-logs/kt156-boot-smp2.log
# smp: boot=0 harts=2 present=0x3 online=0x3 started=0x2 absent=0xfc timeout=0 dtb=0x87e00000 dtb_magic=1 base=0x80400000 stride=0x1d000
```

```text scope=kt156-logs/kt156-boot-smp4.log
# smp: boot=0 harts=4 present=0xf online=0xf started=0xe absent=0xf0 timeout=0 dtb=0x87e00000 dtb_magic=1 base=0x80400000 stride=0x1d000
```

Dòng `# product shell:` đầy đủ (log boot, không cắt):

```text scope=kt156-logs/kt156-boot-smp1.log
# product shell: harts=1 procsrv=0xd shell=0xf victim=0x11 kern=0x13 const:ep_proc=2 const:ep_ps=5 const:ep_events=8 const:slot_cspace=20 const:input_mode=serial const:inject=0
```

**Đọc**: `shell_prompt_lines=2` ở **cả ba** cấu hình hart (dòng `shell>` do shell U-mode in, không
phải kernel); `# shell: up` liệt kê **17** lệnh; `qemu_rc=0` cả ba (thoát sạch sau `exit`).

### 2.2 F2 — chạy hết kịch bản shell + đọc lại tệp **sau reboot**

Ba phần, cùng ảnh đã ghim.

**(a) Parity serial — mỗi lệnh `active` một boot riêng, ở `-smp 1` và `-smp 4`:**

```bash
bash v2/test/parity/run.sh --mode serial --image v2/artifacts/e7149448….bin \
  --smp "$SMP" --logdir v2/agent/JOURNAL/kt156-logs/parity --log-suffix -kt156
```

```text scope=kt156-logs/kt156-f2.log
### parity smp=1 exit=0
# parity: total=28 ok=14 deferred=14 active=14 (active_da_chay=14)
# parity: not_ok=0 timeout=0 khong_co_shell=0 qemu_rc=124
# parity: KET_LUAN=ok (exit 0)
### parity smp=4 exit=0
# parity: total=28 ok=14 deferred=14 active=14 (active_da_chay=14)
# parity: not_ok=0 timeout=0 khong_co_shell=0 qemu_rc=124
# parity: KET_LUAN=ok (exit 0)
```

`qemu_rc=124` là **bình thường**: mỗi boot chỉ nhận **một** lệnh (không nhận `exit`) nên guard tự
kết thúc bằng timeout — cùng dạng với log KT-129 (`kt129-logs/parity/…-smp1-kt129.log` có 13 dòng
`qemu_rc=124`). 14 lệnh `active`: `help echo ps kill cd pwd time ls cat stat sync truncate uptime jobs`.

**(b) Đọc lại tệp sau reboot trên ảnh sản phẩm** — hai tiến trình QEMU **khác nhau**, **cùng** tệp
đĩa 4 MiB (`kt77c-boot.sh`, `-drive`/`virtio-blk-pci`):

```bash
# boot 1: ghi
bash v2/agent/JOURNAL/kt77c-boot.sh v2/artifacts/e7149448….bin "$DISK" 1 \
  v2/agent/JOURNAL/kt156-logs/kt156-f2-write.payload \
  v2/agent/JOURNAL/kt156-logs/kt156-f2-write-smp1.log 300
# boot 2: đọc lại (cùng $DISK)
bash v2/agent/JOURNAL/kt77c-boot.sh v2/artifacts/e7149448….bin "$DISK" 1 \
  v2/agent/JOURNAL/kt156-logs/kt156-f2-read.payload \
  v2/agent/JOURNAL/kt156-logs/kt156-f2-reboot-read-smp1.log 300
```

Payload (nguyên văn, `kt156-f2-write.payload` / `kt156-f2-read.payload`):

```text scope=kt156-logs/kt156-f2-write.payload
write /p.txt hello77
write /p.txt hello77
cat /p.txt
cat /p.txt
ls
ls
exit
```

```text scope=kt156-logs/kt156-f2-read.payload
ls
ls
cat /p.txt
cat /p.txt
exit
```

```text scope=kt156-logs/kt156-f2.log
### f2_write_exit=0
### f2_reboot_read_exit=0
### f2_disk_sha_sau_ghi=cb9ea10368235f0715086cc58a7eafffb6d9bf747f8464299daad138466d91f4
### f2_disk_sha_sau_doc=cb9ea10368235f0715086cc58a7eafffb6d9bf747f8464299daad138466d91f4
### f2_dia_khong_doi_giua_hai_boot=1 (boot 2 chi doc)
### f2_write_markers:
# shell: write /p.txt bytes=7 sync=ok
# shell: write /p.txt bytes=7 sync=ok
# shell: cat ok bytes=7 first=hello77
### f2_reboot_read_markers:
# shell: ls n=2 path=/
# shell: ls n=2 path=/
# shell: cat ok bytes=7 first=hello77
### f2_reboot_read_cat_ok=2
```

**Đọc**: boot 2 (tiến trình QEMU **mới**, cùng đĩa) đọc lại được `hello77` — `# shell: cat ok
bytes=7 first=hello77` **hai lần**; đĩa **không đổi** giữa hai boot (boot 2 chỉ đọc).

**(c) Persist 7 pha** (giữ nguyên phép kiểm của KT-129 — mỗi pha ghi ở một tiến trình, đọc lại ở
tiến trình khác, 10 boot + kiểm host):

```bash
KT75_LOGDIR=v2/agent/JOURNAL/kt156-logs/persist bash v2/test/boot/persist.sh
```

```text scope=kt156-logs/kt156-f2.log
# persist: phases=7 ok=7 boots=10 neg=1 host_checks=10
# persist.sh: PASS
### persist_exit=0
```

### 2.3 F3 — ca âm: ảnh **tắt shell** thì kịch bản phải **đỏ**

**Sai lệch so với đề bài (đo được, không suy đoán).** Đề bài KT-156 ghi build ca âm bằng
`VIFAST_REPRO_CARGO_EXTRA="--features product-fs"` (không có `product-shell`). Lần chạy đầu **theo
đúng đề bài** cho ra **đúng cùng sha** ảnh sản phẩm — vì `product-fs` **kéo theo** `product-shell`:

```text scope=kt156-logs/kt156-f3-lan-dau-sai.log
### build_noshell_exit=0
REPRO machine=Darwin-arm64 root=/private/tmp/night-L9-k7measure sha256=e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71 size=209536 wall_s=15 algo=kt134-v4 inputs=0b8016e6cb6038d5 dirty=0 rev=ba7223da23b8 build=b080e40c1bd815d1 text=130722 image=/private/var/folders/jp/05hgrzxj65bgh355l2zs2zz80000gn/T/vifast-repro-target.P295SD/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
### noshell_sha=e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71
```

Nguyên nhân, đọc thẳng từ `Cargo.toml`:

```text scope=kt156-logs/kt156-f3-cargo-features.log
product-fs = ["product-shell", "vifast-plat-riscv64/block"]
```

Chính comment của `Cargo.toml` nói ca âm đúng là build **không feature nào**:

```text scope=kt156-logs/kt156-f3-cargo-features.log
# `v2/agent/JOURNAL/kt77b-harness-scan.sh`. Ca âm của tiêu chí 3 = build **không** feature này
# (`bash v2/tools/repro-build.sh` mặc định) ⇒ runner parity phải đỏ `exit 5`.
```

Vì vậy F3 được chạy lại bằng [`kt156-f3-negative.sh`](kt156-f3-negative.sh) — build **không
feature**:

```bash
bash v2/agent/JOURNAL/kt156-f3-negative.sh
```

```text scope=kt156-logs/kt156-f3.log
### build_noshell_exit=0
REPRO machine=Darwin-arm64 root=/private/tmp/night-L9-k7measure sha256=1287611cb38c42446c618acf8bba94aba82c50bed53a632656fcb4520ecffe0b size=82552 wall_s=7 algo=kt134-v4 inputs=0b8016e6cb6038d5 dirty=0 rev=ba7223da23b8 build=ec2b9863f3a03853 text=50218 image=/private/var/folders/jp/05hgrzxj65bgh355l2zs2zz80000gn/T/vifast-repro-target.nWSR8z/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
### noshell_sha=1287611cb38c42446c618acf8bba94aba82c50bed53a632656fcb4520ecffe0b
### san_pham_sha=e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71
### noshell_khac_anh_san_pham=1
```

**(a) Ảnh tắt shell KHÔNG in `shell>`** (một boot, cùng payload `help\nexit\n`):

```text scope=kt156-logs/kt156-f3.log
### f3_noshell_boot_exit=0
### f3_noshell_shell_prompt_lines=0
### f3_khong_co_dong_product_shell=1
# build: algo=kt134-v4 inputs=0b8016e6cb6038d5 dirty=0
# k3 halt
```

**(b) Kịch bản của F2 trên ảnh tắt shell phải ĐỎ** — cùng lệnh parity serial:

```text scope=kt156-logs/kt156-f3.log
not ok 1 - help: khong thay shell trong log (qemu_rc=0)
…
not ok 14 - jobs: khong thay shell trong log (qemu_rc=0)
# parity: total=28 ok=0 deferred=14 active=14 (active_da_chay=14)
# parity: not_ok=14 timeout=0 khong_co_shell=14 qemu_rc=0
# parity: KET_LUAN=khong_co_shell (exit 5)
### f3_parity_noshell_exit=5
### f3_parity_noshell_not_ok=14
```

**Đọc**: ảnh tắt shell **không** in `shell>` (0 dòng), **không** có dòng `# product shell:`, và
kịch bản F2 **đỏ** đúng như phải: `not_ok=14`, `khong_co_shell=14`, `KET_LUAN=khong_co_shell`,
exit **5**. Đây là điều kiện để F1/F2 "xanh" có nghĩa.

### 2.4 F4 — ma trận + soak 1.000 trên ảnh đã ghim

**(a) Ma trận** (7 ảnh test × 3 cấu hình hart = 21 boot):

```bash
KT54_LOGDIR=v2/agent/JOURNAL/kt156-logs/ktest-matrix bash v2/test/boot/ktest.sh 1 matrix
```

```text scope=kt156-logs/kt156-f4.log
# KTEST: 21/21 boot sạch (smp=matrix, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec)
# ktest.sh: PASS
### matrix_exit=0
```

**(b) Soak 1.000 boot × 3 luồng trên ảnh sản phẩm đã ghim** (chế độ `--features` của KT-135: mỗi
vòng lái shell và đòi đủ `shell>` + `help` + `exit`/`halt`):

```bash
SOAK_LOGDIR=v2/agent/JOURNAL/kt156-logs/soak bash v2/test/boot/soak.sh \
  --image v2/artifacts/e7149448….bin --features product-shell,product-fs 1000 3
```

```text scope=kt156-logs/kt156-f4.log
# soak: bắt đầu N=1000 lanes=3 16:48:39Z
# soak: xong wall=290s (3 boot/s); ok=1000 fail=0 tổng=1000/1000
# soak: BUILD_ID=b080e40c1bd815d1 sha256=e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71 git=ba7223da23b88c66a922d59e25c1112a104ef1af
# soak: 1000/1000 lỗi=0 — 0/N chỉ là trần (QD §5.3); không retry (D2-07)
# soak: 0/1000 lỗi (PASS)
# soak.sh: PASS
### soak_exit=0
```

Bằng chứng **mỗi vòng** đủ ba dấu hiệu (trích boot 1 trong 1.000):

```text scope=kt156-logs/soak/run-20261119-014839-31615/boot-1.log
# build: algo=kt134-v4 inputs=0b8016e6cb6038d5 dirty=0
shell>
# shell: help cmds=17
shell>
# shell: exit code=0
# product halt: shell_exit=0 fault_seen=1 events=3 ps_replies=0 ps_n=0 kill_suspend=0 proc_up=1 fs_replies=0 rx_windows=38 rx_irqs=2 rx_bytes=10 rx_late=2 ring_used=0 ring_head=3 ring_tail=3 ring_ok=1
```

`results.txt` của soak: **0** dòng `fail` (`grep -c '^fail ' …/results.txt` → `0`), dòng cuối
`ok 1000`.

**Ghi chú về "8 ảnh × 3 = 24 boot"**: `ktest.sh 1 matrix` chạy **7** ảnh test × 3 hart = **21**
boot (đúng danh sách `expected.txt` hiện tại: `ktest ktest-guard ktest-user ktest-proc ktest-vblk
ktest-con ktest-exec`). Con số **24** trong OD-K6-2 là của thời điểm có **8** ảnh; cây hôm nay có
**7** ảnh test. Ảnh sản phẩm **không** nằm trong ma trận `ktest.sh` — nó được đo riêng ở F1/F2/F5/F6
và ở soak (đúng thiết kế, xem `K6-CLOSURE.md` §4.3). **Không** nới/không đổi phạm vi: báo cáo đúng
số đo được (21/21) và nêu rõ vì sao khác 24.

### 2.5 F5 — ngưỡng giữ nguyên

```bash
bash v2/test/boot/budget.sh v2/artifacts/e7149448….bin product
```

```text scope=kt156-logs/kt156-f5.log
### image_bytes=209536 tran=262144
# image=product path=/private/tmp/night-L9-k7measure/v2/artifacts/e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71.bin file=209536 data+bss=223696 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
# sections-seen: .text=130722 .rodata=64024 .data=7832 .bss=215864 .stack_guard=4096 .stack=65536 .trap_stack=16384 .syscall_stack=32768 .hart_stacks=831488 .riscv.attributes=116 
# data+bss-sections: .data=7832 .bss=215864 
### budget_exit=0
```

**Đọc**: `file=209536` ≤ `budget=262144`; `data+bss=223696` ≤ `262144`; `budget_exit=0`. Ngưỡng
**không đổi** (`BUDGET_BYTES=$((256 * 1024))` trong `budget.sh`).

### 2.6 F6 — ảnh sản phẩm không chứa harness (quét bằng máy)

Dùng đúng script KT-129 đã dùng — [`kt77b-harness-scan.sh`](kt77b-harness-scan.sh) (15 marker chuỗi
+ lớp feature-graph `probe`), **bắt buộc** có ca âm (OD-K6-5 luật 3):

```bash
FEAT_PRODUCT="product-shell,product-fs" FEAT_HARNESS="ktest,user" \
  bash v2/agent/JOURNAL/kt77b-harness-scan.sh \
    --image v2/artifacts/e7149448….bin --negative v2/agent/JOURNAL/kt156-logs/ktest-proc-ca-am.bin
```

Ảnh ca âm được build trong script (`--features ktest,user`, bin `vifast-kbin-ktest-proc`).

```text scope=kt156-logs/kt156-f6.log
# kt77b-harness-scan: image=/private/tmp/night-L9-k7measure/v2/artifacts/e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71.bin sha256=e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71 bytes=209536
# kt77b-harness-scan: markers=15 (chuỗi do mã test phát ra; ảnh canonical đã strip)
# scan product: hits=0
ok 1 - anh san pham khong chua ma harness (hits=0)
# kt77b-harness-scan: negative=/private/tmp/night-L9-k7measure/v2/agent/JOURNAL/kt156-logs/ktest-proc-ca-am.bin sha256=72b5cc869f69385bfaad40c1f5c822e537d6102a2e7854b7f17e69b8a40db940 bytes=438696
# marker 'ktest_' hits=2
# marker 'proc_demo' hits=2
# marker '# ktest halt' hits=1
# marker 'not ok' hits=19
# marker 'probe_fault_entry' hits=1
# scan negative: hits=25
ok 2 - ca am: anh co harness bi bat (hits=25 > 0)
# kt77b-harness-scan: feature-graph product=product-shell,product-fs harness=ktest,user (crate vifast-plat-riscv64)
# feat product: ktest=0 umode=1
# feat harness: ktest=1
ok 3 - probe gate: cau hinh san pham KHONG bat 'ktest' (probe khong duoc bien dich), co 'umode'
ok 4 - ca am cua probe gate: cau hinh anh test BAT 'ktest' (ktest=1 > 0)
# kt77b-harness-scan: KET_LUAN=ok (product=0, negative>0, probe_gate=ok)
### harness_scan_exit=0
```

**Đọc**: ảnh sản phẩm **0** hit trên **15** marker; ca âm **25** hit (bộ dò **không** rỗng); lớp
feature-graph: cấu hình sản phẩm `ktest=0` (không biên dịch `probe`), cấu hình ảnh test `ktest=1`.

## 3. Thời gian từng phép đo

Mốc lấy từ header mục trong `kt156-master.log` (mỗi mục in `date -u` lúc bắt đầu):

| Phép đo | Bắt đầu (UTC) | Kết thúc (UTC) | Thời gian |
|---|---|---|---|
| F1 (3 boot 1/2/4 hart) | 16:12:33 | 16:12:39 | **6 s** |
| F6 (build ảnh ca âm + quét) | 16:12:39 | 16:12:55 | **16 s** |
| F5 (budget) | 16:12:55 | 16:12:55 | **< 1 s** |
| F2 (parity 2×14 boot + reboot-read + persist 10 boot) | 16:12:55 | 16:34:43 | **21 m 48 s** |
| F3 lần đầu (sai — trùng sha, §2.3) | 16:34:43 | 16:45:38 | 10 m 55 s |
| F4 ma trận (21 boot) | 16:45:38 | 16:47:36 | **1 m 58 s** |
| F4 soak (1.000 boot, 3 luồng) | 16:47:36 | 16:53:30 | **5 m 54 s** (`wall=290s` do soak tự đo) |
| **Toàn script** | 16:12:32 | 16:53:30 | **40 m 58 s** |
| F3 chạy lại (đúng, §2.3) | 16:54:17 | 16:54:41 | **24 s** |

## 4. Hạ tầng trước/sau

```text scope=kt156-logs/kt156-master.log
### infra truoc qemu_total=0 loadavg=1.91 2.84 3.50 ncpu=12
```

```text scope=kt156-logs/kt156-master.log
### infra sau qemu_total=0 loadavg=5.29 4.99 4.54 ncpu=12
```

Soak tự kiểm hạ tầng **sau** phép đo (OD-K6-6):

```text scope=kt156-logs/kt156-f4.log
# infra after: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=5.29 loadavg_5m=4.99 loadavg_15m=4.54 ncpu=12 boot_done=1 verdict=OK
# infra: invalid=0 isolation_bad=0 guard_log=/private/tmp/night-L9-k7measure/v2/agent/JOURNAL/kt156-logs/soak/qemu-guard.log
```

F3 (chạy riêng, sau khi script chính xong):

```text scope=kt156-logs/kt156-f3.log
### qemu_con_sot_truoc=0
# parity: qemu_con_sot_total=1 qemu_cua_script=0
### qemu_con_sot_sau=0
```

`qemu_con_sot_total=1 qemu_cua_script=0` = lúc đo có **1** QEMU của **agent khác** trong cùng đêm
chạy song song; **không** phải của script này (`qemu_cua_script=0`), và trước/sau F3 đều **0**.
Không `pkill` toàn cục; chỉ `kill` PID do guard tạo.

## 5. Phát hiện — đề bài F3 sai một chỗ (đã đo, đã sửa)

Đề bài KT-156 viết: *"build **lần hai** với `VIFAST_REPRO_CARGO_EXTRA="--features product-fs"`
(không có `product-shell`)"*. Trong cây này **câu đó không tạo ra ảnh tắt shell**:

- `v2/kernel/vifast-kbin-riscv64/Cargo.toml:29`: `product-fs = ["product-shell", "vifast-plat-riscv64/block"]`
  ⇒ bật `product-fs` là **kéo theo** `product-shell`.
- Bằng chứng đo: lần chạy đầu ra `sha256=e7149448…` — **trùng** ảnh sản phẩm (log
  `kt156-f3-lan-dau-sai.log`), và boot nó vẫn in `shell>` (`f3_noshell_shell_prompt_lines=2`).
- Ca âm đúng (theo chính comment `Cargo.toml`) là build **không feature nào** ⇒
  `sha256=1287611c…`, 82.552 B, **0** dòng `shell>`, parity **đỏ** `exit 5`.

Đã sửa **script** (`kt156-closure-measure.sh` mục F3 dùng build không feature) và chạy lại bằng
`kt156-f3-negative.sh`. Log lần đầu **giữ nguyên** trong `kt156-f3-lan-dau-sai.log` để người đọc
thấy phép đo, không xoá.

## 6. Hạn chế — ghi nguyên văn, không giấu

1. **Ma trận là 21 boot, không phải 24**: cây hôm nay có **7** ảnh test (`expected.txt`), không
   phải 8. Ma trận **không** chứa ảnh sản phẩm (đúng thiết kế — ảnh sản phẩm đo ở F1/F2/F5/F6 +
   soak). Không thêm ảnh giả để đủ 24.
2. **Parity serial mỗi boot kết thúc bằng timeout của guard** (`qemu_rc=124`) vì mỗi boot chỉ nhận
   **một** lệnh, không nhận `exit`. Đây là hành vi của runner (giống KT-129), không phải boot treo:
   `not_ok=0 timeout=0`.
3. **F2c persist chạy trên ảnh test `ktest-persist`**, không phải ảnh sản phẩm — `persist.sh` điều
   khiển bằng "plan" ghi vào LBA 0 của đĩa, cơ chế chỉ ảnh test có. Đọc-lại-sau-reboot **trên ảnh
   sản phẩm** là phần F2b (payload serial + `-drive`), không phải persist.sh.
4. **Soak dùng chế độ `--features` của KT-135** (harness chính thức, lái shell mỗi vòng) — không
   dùng script riêng `kt90-soak-product-image.sh` như thời K6 đóng. Đây là đường đo **mạnh hơn**
   (cùng harness, cùng ảnh ghim), không phải nới.
5. **`v2/artifacts/` bị `.gitignore`** (`artifacts/`) ⇒ ảnh ghim **không** vào git; bằng chứng là
   sha trong log + dòng `# build:`. Ai muốn tái lập phải build lại từ HEAD `ba7223da` với
   `--features product-shell,product-fs`.

## 7. Danh mục log

| Tệp | Nội dung |
|---|---|
| `kt156-master.log` | toàn bộ stdout của `kt156-closure-measure.sh` (nguyên văn) |
| `kt156-f1.log` | F1 — 3 boot 1/2/4 hart + REV-GATE |
| `kt156-f2.log` | F2 — parity serial smp1/smp4 + reboot-read + persist |
| `kt156-f3.log` | F3 — build ảnh tắt shell + boot + parity đỏ (bản **đúng**) |
| `kt156-f3-lan-dau-sai.log` | F3 lần đầu theo đề bài — **trùng sha**, giữ làm bằng chứng §5 |
| `kt156-f4.log` | F4 — ma trận 21 boot + soak 1.000 |
| `kt156-f5.log` | F5 — budget |
| `kt156-f6.log` | F6 — quét harness + ca âm + feature-graph |
| `kt156-build-product.log` / `.out` | log build ảnh sản phẩm (pipeline canonical) |
| `kt156-build-noshell.log` | log build ảnh ca âm F3 |
| `kt156-boot-smp{1,2,4}.log` | log boot F1 (nguyên văn) |
| `kt156-f2-write-smp1.log`, `kt156-f2-reboot-read-smp1.log` | F2b — ghi rồi đọc lại sau reboot |
| `parity/` | log từng lệnh parity (F2 + F3) |
| `persist/` | log 10 boot persist + kiểm host |
| `ktest-matrix/` | log 21 boot ma trận |
| `soak/run-20261119-014839-31615/` | 1.000 log boot + `results.txt` + `summary` của soak |
| `ktest-proc-ca-am.bin` | ảnh ca âm F6 (có harness) |

**Ghi chú về tệp vào git**: `v2/artifacts/` bị `.gitignore` (`artifacts/`) nên **ảnh ghim không vào
git** — bằng chứng là sha trong log + dòng `# build:`. Trong `kt156-logs/`, các tệp **nhị phân/lớn**
(`*.img` 4 MiB, `ktest-proc-ca-am.bin` 428 KiB) và **999 log boot soak** (chỉ giữ `boot-1.log` làm
mẫu) **không** vào git; chúng vẫn nằm trên đĩa worktree và đều **sinh lại được** bằng
`kt156-closure-measure.sh` (ảnh ca âm F6 do script tự build nếu thiếu). Mọi tệp **được trích trong
báo cáo** đều đã vào git.
