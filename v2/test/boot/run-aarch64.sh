#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-338c** (D2-08/D2-244) — runner này kiểm **hai ảnh** từ **một** crate:
#   - **sản phẩm** (build **không** feature): `strings` phải **0 harness marker** (danh sách
#     `PRODUCT_HARNESS_MARKERS`), boot tới marker + halt + `# conf image:` khớp ELF, và log sản
#     phẩm **phải bị** bộ kiểm TAP từ chối (nếu không, "bộ kiểm sản phẩm" là dấu cao su);
#   - **ktest** (`--features ktest`): giữ **nguyên** TAP 35 + 24 ca âm cũ, cộng ca âm **(y)** của
#     hộp thư IPI, cộng **KT-338b** — TAP **36..41** (`vifast-core::Machine` + IPC từ EL0 qua
#     `svc #0`) và ca âm **(z)** "thiếu dòng `# core ipc:`" ⇒ **41 TAP**, **26 ca âm**.
# Cộng **hộp thư IPI bitmask dùng chung** (D2-244 tầng 1): dòng `# ipi: mailbox posted=… coalesced=…
# sent=… drained=… empty=0 sgi=0 target=1` là bằng chứng **máy kiểm** rằng SGI của aarch64 đã đi
# qua `vifast_plat_common::ipi::IpiMailbox` (thay vì chỉ đếm IPI).
# KT-250 — runner boot **aarch64** (K4' bước 3, B3.8 phần đầu). Khuôn: `run.sh` (riscv64) +
# `qemu_guard.sh` (KT-78: process group + timeout + dọn con).
#
# Việc runner làm, theo thứ tự:
#   1. **build** ảnh `vifast-kbin-aarch64` cho `aarch64-unknown-none` (release);
#   2. **kiểm ELF** (T-objdump của thiết kế): `.text` nạp đúng `0x4008_0000`; `.text.vectors` là
#      `TEXT` **không** phải `BSS` (bất biến của B3.2); 16 stub `__vec_i` nằm đúng
#      `__vector_base + i*0x80`;
#   3. **boot** QEMU `virt` N lần qua `qemu_guard.sh`, kiểm **TAP** (`TAP version 13`, `1..N`,
#      đúng N dòng `ok`, không `not ok`, không `PANIC`) + marker **đúng một lần**;
#   4. in `ok`/`not ok` cho từng kiểm, exit 0 ⇔ mọi kiểm xanh.
#
# Dùng: bash v2/test/boot/run-aarch64.sh [NBOOT]
#
# **Ca âm chạy MẶC ĐỊNH** (không cần cờ): sau lần boot đầu, runner chạy lại **cùng** bộ kiểm
# `check_log` trên log tốt với 11 kỳ vọng **sai** (marker sai, thiếu một `ok`, có `not ok`, plan
# sai, log rỗng, thiếu dòng hart 1 online, IPI in hai lần, thiếu dòng `# tlbi:`, thiếu ack
# shootdown, **hash ảnh lệch ELF**, **thiếu dòng `# umode:`**) và đòi **cả 11 phải ĐỎ**; cộng
# **ca âm (l) của KT-273** — boot **không có đĩa** phải in `# blk: no device` và TAP phải ĐỎ;
# cộng **ca âm (m) của KT-275** — boot **không có mạng** phải in `# net: no device` và TAP phải
# ĐỎ; cộng **hai ca âm của KT-277** — thiếu dòng `# wx:` (W^X) và thiếu dòng `# dma:` (cache
# maintenance) đều phải ĐỎ; cộng **ca âm (p) của KT-279** — boot **không có GPU** phải in
# `# gpu: no device` và TAP phải ĐỎ; cộng **ca âm (q) của KT-283** — boot **không có bàn phím**
# phải in `# input: no device` và TAP phải ĐỎ; cộng **hai ca âm (r)/(s) của KT-289** — thiếu dòng
# `# net: tcp`/`# net: dns` đều phải ĐỎ; cộng **hai ca âm (t)/(u) của KT-294** — log có
# `# net: no dns` (cả hai lần thử timeout) và log có `attempts=3` (retransmit vô hạn) đều phải ĐỎ;
# cộng **hai ca âm (v)/(w) của KT-302** — log có `buses=16` (quét thiếu bus) và log có
# `ecam_block_ok=0` (ECAM map bằng lá 4 KiB thay block 2 MiB) đều phải ĐỎ; cộng **ca âm (x) của
# KT-305** — log có `blocks_split=0` (unmap block **không** chia nhỏ) phải ĐỎ (**24 ca âm**). Nhờ vậy
# mỗi lần chạy runner vừa
# chứng minh ảnh boot sạch, vừa chứng minh bộ kiểm **không phải dấu cao su** — không có đường
# "PASS giả".
#
# Biến môi trường (dùng cho ca âm và cho đo lại):
#   EXPECT_MARKER   dòng marker phải xuất hiện đúng 1 lần (mặc định `# viFastOs aarch64: boot ok`)
#   EXPECT_TESTS    số ca TAP (mặc định 35 — KT-253 thêm 2 ca nửa cao/physmap, KT-254 thêm 2 ca
#                   VSpace/fault, KT-259 thêm 3 ca SMP: hart 1 online, IPI 0->1, cả hai hart báo,
#                   KT-262 thêm 3 ca TLBI/smp 4, KT-269 thêm 3 ca U-mode: EL0 vào được, `svc`
#                   được xử lý, EL0 chạy tiếp sau `svc` rồi `Exit`, **KT-273 thêm 3 ca PCI/blk**:
#                   `pci: ecam`, `pci: virtio-blk found`, `blk: sector0`, **KT-275 thêm 4 ca
#                   mạng**: `net: device`, `net: dhcp`, `net: arp`, `net: ping`, **KT-277 thêm 2
#                   ca**: `wx: text RX (write faults), data RW`, `dma: cache maintenance
#                   before/after DMA`, **KT-279 thêm 3 ca GPU**, **KT-283 thêm 2 ca bàn phím**,
#                   **KT-289 thêm 3 ca TCP/DNS**: `net: tcp connect`, `net: tcp data`, `net: dns`)
# cộng **hai ca âm của KT-277** — thiếu dòng `# wx:` (W^X) và thiếu dòng `# dma:` (cache
# maintenance) đều phải ĐỎ (14 ca âm). Nhờ vậy mỗi lần chạy runner vừa chứng minh ảnh boot sạch,
# vừa chứng minh bộ kiểm **không phải dấu cao su** — không có đường "PASS giả".
#
# Biến môi trường (dùng cho ca âm và cho đo lại):
#   EXPECT_MARKER   dòng marker phải xuất hiện đúng 1 lần (mặc định `# viFastOs aarch64: boot ok`)
#   EXPECT_TESTS    số ca TAP (mặc định 23 — KT-253 thêm 2 ca nửa cao/physmap, KT-254 thêm 2 ca
#                   VSpace/fault, KT-259 thêm 3 ca SMP: hart 1 online, IPI 0->1, cả hai hart báo,
#                   KT-262 thêm 3 ca TLBI/smp 4, KT-269 thêm 3 ca U-mode: EL0 vào được, `svc`
#                   được xử lý, EL0 chạy tiếp sau `svc` rồi `Exit`, **KT-273 thêm 3 ca PCI/blk**:
#                   `pci: ecam`, `pci: virtio-blk found`, `blk: sector0`, **KT-277 thêm 2 ca**:
#                   `wx: text RX (write faults), data RW`, `dma: cache maintenance before/after DMA`)
#   EXPECT_TEXT_VMA VMA của `.text` (mặc định 0x40080000)
#   KT_LOGDIR       thư mục log (mọi runner); KT250_LOGDIR riêng tệp này.
#                   Mặc định: $V2_RUN_TMP (nếu có) hoặc $TMPDIR/k4-aarch64.<pid>
#
# **KT-259**: QEMU chạy **2 hart** (`-smp 2`) và **GICv3** (`-M virt,gic-version=3`) — mặc định
# của QEMU `virt` là GICv2 (`arm,cortex-a15-gic`, đo bằng `-M virt,dumpdtb` + `dtc`), không có
# redistributor/`ICC_*_EL1` nên IPI `SGI 0` không thể chạy. Ngoài TAP, runner còn đòi **đúng một**
# dòng bằng chứng cho mỗi hart (`# smp: hart=1 online`, `# smp: ipi recv=1`, `# smp: hart=0 alive`,
# `# smp: hart=1 alive`) — TAP nói phép đo đúng, các dòng ấy nói **hart nào** đã in.
#
# **KT-262**: `-smp 4` (bốn hart) + **shootdown `TLBI`**. Runner đòi thêm:
#   - `# smp: hart=2 online`, `# smp: hart=3 online`, `# smp: hart=2 alive`, `# smp: hart=3 alive`
#     — đúng **một** lần mỗi dòng (một ảnh chỉ chạy 2 hart vẫn in `ok 12` nhưng **không** có các
#     dòng này ⇒ ĐỎ);
#   - `# tlbi: shootdown … acks=<N-1> expected=<N-1> ok=1` — **bằng chứng ack** của shootdown;
#   - `# tlbi: hart=1 read1=… match=1` — hart 1 **đã nạp** entry vào TLB trước khi `unmap`;
#   - `# tlbi: hart=1 read2 … fault=1` — hart 1 **fault** sau `unmap` ⇒ TLB của nó đã bị xoá.
#
# **Không** tuyên bố K4' đạt: K4' = "như K3" (TAP đầy đủ smp 1|2|4 + soak 1.000) — bước sau.
# Runner này là B3.8 + B3.9 + **B3.6** (bốn hart, một ảnh, TAP 15 ca sau KT-262).
#
# **KT-265**: ảnh in thêm `# conf image: build=<16 hex> text=<len>` (FNV-1a 64 trên `.text` đã
# nạp, D2-80) — runner băm lại **cùng** section từ ELF (`test/conformance/image_digest.py`) và
# đòi log khớp **từng trường**; lệch/thiếu ⇒ boot đó ĐỎ. Đây là bất biến `soak-aarch64.sh` dùng
# chung, nên runner và soak không thể lệch nhau về định nghĩa "ảnh nào".
#
# **KT-273**: QEMU nhận thêm một **đĩa tạm** có nội dung **biết trước**
# (`-drive file=<tmp>,if=none,id=d0,format=raw,snapshot=on -device virtio-blk-pci,drive=d0`;
# 8 sector 512 B, 17 byte đầu `VIFASTOS-KT273-S0`). `snapshot=on` để QEMU **không** giữ khoá ghi
# trên tệp đĩa — soak chạy 3 luồng song song trên **cùng** tệp (nếu thiếu, QEMU báo
# `Failed to get "write" lock`, đo được khi dựng KT-273). và `-nic none` để **topology PCI tất định** (chỉ host bridge +
# virtio-blk). Runner đòi thêm ba dòng bằng chứng:
#   - `# pci: ecam … class=0x060000 … empty=0xffff …` — ECAM map đúng và giải mã thật;
#   - `# pci: virtio-blk found … vendor=0x1af4 … assigned=… readback=…` — BAR gán rồi **đọc lại**
#     đúng;
#   - `# blk: sector0 hex=<32 hex> … magic=1` — hex phải khớp **16 byte đầu của đĩa tạm** (runner
#     băm lại từ tệp bằng `od`), nên "đọc được sector 0" không thể là đọc ra số 0.
#
# **KT-279**: QEMU nhận thêm **card đồ hoạ** (`-device virtio-gpu-pci`) và runner mở **HMP
# monitor** trên unix socket (`-monitor unix:<sock>,server=on,wait=off`) để `screendump` **trong
# lúc khách đang chạy**. Ảnh in `# gpu: console done lines=3` rồi **giữ khung hình 500 ms**
# (`gpu::GPU_HOLD_MS`) — runner chờ **marker của khách** (không `sleep` mù), chụp PPM, rồi chờ
# QEMU tự thoát. `gpu_ppm.py` (dùng **nguyên** bản của riscv64, không sửa) đòi **đúng màu tại
# đúng toạ độ** + **từng ký tự** của ba dòng console so bitmap 8×8 đọc từ
# `v2/lib/vifast-virtio/src/gpu.rs`. Runner đòi thêm các dòng bằng chứng:
#   - `# gpu pci: … vendor=0x1af4 device=0x1050 … assigned=… readback=…` — BAR gán rồi **đọc lại**
#     đúng, ở leaf **riêng** của GPU (`0x1080_0000`, không đụng leaf blk/net);
#   - `# gpu dev: … version1=1 device_status=0x0f num_scanouts=1`;
#   - `# gpu fb: pa=0x42000000 walk=0x42000000 identity=1 bytes=1228800 pages=300` — framebuffer
#     **ngoài ảnh**, map identity (PA **đọc lại từ bảng trang**), nên `.data+.bss` **không** tăng
#     vì framebuffer (trần I7 262.144 B **không** bị nới);
#   - `# gpu scanout: … bogus=InvalidResourceId` — ca âm đối chứng: thiết bị **kiểm**
#     `resource_id`, nên "SET_SCANOUT thành công" không phải lệnh rỗng;
#   - `# gpu console: … chars=29 newlines=3 scrolls=0 dirty=8,8,88,24 flushes=1 commands=2
#     matched=29/29 text=ViFastOs|gpu console|0123456789` — flush **một lô** = đúng **hai** lệnh;
#   - `# gpu stats: … errors=0 timeouts=0 id_mismatch=0 rejects=1 … fb_clean_lines=<n> …` — bộ đếm
#     cache maintenance của framebuffer **đã chạy** (`n > 0`).
#
# **KT-283**: QEMU nhận thêm **bàn phím** (`-device virtio-keyboard-pci`) và runner bơm phím qua
# **HMP monitor** (`sendkey a`) — cùng kỹ thuật `input.sh` riscv64 (KT-255), không `sleep` mù: mốc
# là dòng `# input: ready` của **khách** (ảnh in **sau** khi đã arm 8 đệm; bơm sớm là mất lô).
# Runner đòi thêm các dòng bằng chứng:
#   - `# input pci: … vendor=0x1af4 device=0x1052 … assigned=0x10600000 readback=0x10600000` — BAR
#     gán rồi **đọc lại** đúng, ở leaf **riêng** của input (`0x1060_0000`, không đụng blk/net/gpu);
#   - `# input name: QEMU Virtio Keyboard` — tên **đọc từ** `virtio_input_config`;
#   - `# input dev: … version1=1 rep_feature=0 device_status=0x0f num_queues=2 queue_size=16
#     status_queue_size=16 key_a_bit=1` — feature VERSION_1 hai chiều, **không** `REP`, **hai**
#     queue `event`/`status`, bitmap `EV_BITS` bật `KEY_A`;
#   - `# input dma: … identity=1 armed=8 status_queue_idle=1` — trang DMA map identity (PA **đọc
#     lại từ bảng trang**), 8 đệm arm sẵn, queue `status` bật nhưng **không** arm đệm;
#   - `# input: ready` đúng một lần (bằng chứng runner bơm phím **sau** khi arm);
#   - `# input: key a code=30 ascii=0x61 shift=0` — `sendkey a` ⇒ `EV_KEY KEY_A 1` ⇒ `'a'`;
#   - `# input stats: … errors=0 timeouts=0 id_mismatch=0 clean_lines=<n> inval_lines=<n>` — bộ đếm
#     cache maintenance **đã chạy** (`n > 0`; bỏ `dc` ⇒ 0 ⇒ ĐỎ).
#
# **KT-275**: QEMU nhận thêm **card mạng** slirp
# (`-netdev user,id=n0 -device virtio-net-pci,netdev=n0`; `-nic none` vẫn giữ để **không** có NIC
# mặc định thứ hai — topology PCI tất định: host bridge + virtio-net + virtio-blk). Runner đòi
# thêm bốn dòng bằng chứng:
#   - `# net: device … vendor=0x1af4 … mac=<6 octet> …` — MAC **đọc từ `virtio_net_config`**,
#     BAR gán rồi đọc lại đúng;
#   - `# net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2` — DHCP **thật**
#     của slirp (cùng khuôn `net.sh` riscv64);
#   - `# net: arp gw=<mac>` — ARP reply **thật** của slirp (MAC khác 0/broadcast);
#   - `# net: ping 10.0.2.2 ok n=3 rtt=<ms>` — ICMP echo **thật** qua gateway.
#
# **KT-289**: TCP client + DNS trên aarch64 (khuôn `net.sh` riscv64 KT-268). Runner dựng **server
# TCP thật** bằng `python3` trên `127.0.0.1:45678`; khách nối `10.0.2.2:45678` — slirp ánh xạ
# `10.0.2.2` → loopback host (`sotranslate_out4`). **Không** dùng `hostfwd`: nó là chiều
# host → guest, không dùng được cho chiều guest → host (KT-268 đã đo). Server ghi log
# `listen`/`accept`/`sent`/`recv` để runner có bằng chứng **hai chiều** khách ↔ host. Runner đòi
# thêm ba dòng bằng chứng:
#   - `# net: tcp connect 10.0.2.2:45678 ok rtt=<ms> state=established` — bắt tay ba bước **thật**
#     với server trên host;
#   - `# net: tcp recv=13 first=vifast-tcp-ok` + `# net tcp data: recv=13 sent=15 send=ok close=ok
#     …` — nhận **đúng** chuỗi server gửi, gửi lại payload, đóng bằng FIN;
#   - `# net: dns example.com a=<ip> ttl=<n>` — truy vấn DNS **thật** tới `10.0.2.3:53` của slirp.
# Cộng **hai ca âm** cho hai dòng `# net: tcp`/`# net: dns` (thiếu dòng ⇒ ĐỎ) — **19 ca âm**;
# **KT-294** thêm **hai** ca âm nữa (cả hai lần thử timeout; `attempts=3`) — **21 ca âm**;
# **KT-302** thêm **hai** ca âm nữa (`buses=16`; `ecam_block_ok=0`) — **23 ca âm**;
# **KT-305** thêm **một** ca âm nữa (`blocks_split=0`) — **24 ca âm**.
#
# **KT-302**: ECAM 256 MiB map bằng **block descriptor 2 MiB ở mức 2** (128 block trong **một**
# bảng mức 2 ⇒ 2 frame) thay vì 128 bảng mức 3 (129 frame — vượt arena 44 frame + trần I7).
# Runner đòi thêm hai điều kiện **đo được**:
#   - `# pci: ecam … buses=256` — quét **đủ** 256 bus (trước KT-302 chỉ 16);
#   - `# pci: map … ecam_blocks=128 ecam_block_level=2 ecam_block_ok=1` — `main.rs` walk bảng
#     trang **thật** tại VA đầu cửa sổ ECAM và thấy `Block { pa: ECAM_BASE, level: 2 }`; nếu map
#     bằng lá 4 KiB thì `ecam_block_ok=0` ⇒ ĐỎ.
#
# **KT-305**: `translate`/`unmap` **hiểu** block descriptor (trước KT-305 cả hai trả
# `BlockUnsupported`). Runner đòi thêm dòng **đo được** `# pci: unmap … level=2 translate_ok=1
# split=1 neighbor_ok=1 unmapped_ok=1 remap_ok=1 blocks_split=1` — `ecam.rs` đo trên bảng trang
# thật: translate trả PA + mức 2, unmap **chia nhỏ** block (trang kế cận còn map), map lại được.
# `blocks_split=0` (xoá cả block) ⇒ ĐỎ.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
TARGET="aarch64-unknown-none"
IMAGE="$V2_ROOT/kernel/target/$TARGET/release/vifast-kbin-aarch64"
# **KT-338c** (D2-08) — **hai** ảnh từ **một** crate, khác feature (cùng khuôn `run-x86_64.sh`
# của KT-340a):
#   - ảnh **sản phẩm** `$PRODUCT_IMAGE`: build **không** feature `ktest` ⇒ 0 harness marker;
#   - ảnh **kiểm** `$KTEST_IMAGE`: build `--features ktest` ⇒ TAP 35 (giữ nguyên hợp đồng cũ).
# Runner build **cả hai**, giữ bản sao trong `$LOGDIR` để ảnh kiểm không đè ảnh sản phẩm; mọi
# phép kiểm TAP bên dưới chạy trên ảnh kiểm (đúng nghĩa "ảnh test không nằm trong ảnh sản phẩm").
# Dòng halt của ảnh **sản phẩm** (in sau marker, trước PSCI `SYSTEM_OFF`).
HALT_LINE="# viFastOs aarch64: halt"
# Marker **harness** (do mã kiểm phát ra) — ảnh sản phẩm phải **0**; ảnh ktest phải **> 0**
# (ca âm của chính bộ quét, OD-K6-5 luật 3). Không đưa `# conf image:` vào đây: nó là bằng chứng
# **sản phẩm** (D2-80), không phải harness.
PRODUCT_HARNESS_MARKERS=(
    'TAP version 13' 'not ok' '# TAP ' 'ok 1 - ' '# smp:' '# net:' '# umode:' '# blk:' '# gpu:'
    '# input:' '# pci:' '# tlbi:' '# wx:' '# dma:' '# ipi:' '# vspace:' '# high:' '# mmu:'
    '# brk:' '# trap:' '# vector:' '# fault:' '# physmap:'
)
# KT-265: bản băm `.text` từ tệp ELF — nguồn đối chiếu của dòng `# conf image:` do kernel in.
DIGEST="$V2_ROOT/test/conformance/image_digest.py"
N_BOOT=1
TIMEOUT_DS=200 # 20 s/boot (cùng ngân sách `run.sh` riscv64)
# **KT-333c** (D2-243 mục 3b) — logdir **riêng theo phiên** (`$V2_RUN_TMP` hoặc
# `$TMPDIR/k4-aarch64.<pid>`); ép bằng `KT_LOGDIR`/`KT250_LOGDIR`. Lý do như `run-x86_64.sh`.
LOGDIR="${KT_LOGDIR:-${KT250_LOGDIR:-${V2_RUN_TMP:-${TMPDIR:-/tmp}}/k4-aarch64.$$}}"
PRODUCT_IMAGE="$LOGDIR/vifast-kbin-aarch64-product"
KTEST_IMAGE="$LOGDIR/vifast-kbin-ktest-aarch64"
if [ "${1:-}" = "--print-logdir" ]; then
    echo "$LOGDIR"
    exit 0
fi
EXPECT_MARKER="${EXPECT_MARKER:-# viFastOs aarch64: boot ok}"
EXPECT_TESTS="${EXPECT_TESTS:-41}"
EXPECT_TEXT_VMA="${EXPECT_TEXT_VMA:-0x40080000}"
# **KT-262**: số hart QEMU (`-smp`) và số hart phụ phải ack shootdown = `EXPECT_SMP - 1`.
EXPECT_SMP="${EXPECT_SMP:-4}"
EXPECT_ACKS=$((EXPECT_SMP - 1))
# **KT-279**: PPM + monitor. `gpu_ppm.py` là **nguyên** bản của riscv64 (KT-263/KT-271) — cùng
# hình học 640×480, cùng bốn điểm đỏ, cùng 29 ký tự console; không có bản thứ hai cho aarch64.
PPM_CHECK="$HERE/gpu_ppm.py"
# Số bước 0,1 s chờ marker `# gpu: console done lines=3` trước khi coi là ảnh không lên được.
READY_DS=100
# Địa chỉ BAR driver **gán** cho GPU = leaf thứ 5 của cửa sổ PCIe (`PCI_MMIO_WINDOW_BASE` +
# 4 × 2 MiB). Đây là **hằng thiết kế** của driver (không phải số đo), runner đối chiếu để bắt
# "gán nhầm leaf của blk/net".
GPU_BAR_ASSIGNED='0x10800000'
# **KT-283**: địa chỉ BAR driver **gán** cho virtio-input = leaf thứ 4 của cửa sổ PCIe
# (`PCI_MMIO_WINDOW_BASE` + 3 × 2 MiB). Hằng thiết kế của driver (không phải số đo) — runner đối
# chiếu để bắt "gán nhầm leaf của blk/net/gpu".
INPUT_BAR_ASSIGNED='0x10600000'
# Socket monitor — tên ngắn (giới hạn 104 byte của unix socket trên macOS). **KT-283**: cùng
# socket phục vụ `screendump` (GPU) và `sendkey` (bàn phím).
SOCK="${TMPDIR:-/tmp}/kt283-mon-$$.sock"
# **KT-289** — TCP: server **thật** trên host. Runner dựng `python3` lắng nghe
# `127.0.0.1:$TCP_PORT`; khách nối `10.0.2.2:$TCP_PORT` (slirp ánh xạ 10.0.2.2 → loopback host —
# xem ghi chú `hostfwd` ở đầu tệp). Server gửi đúng `$TCP_TOKEN` (13 byte) rồi chờ khách gửi
# `$TCP_SEND_LEN` byte + FIN; log server là bằng chứng **hai chiều**.
TCP_PORT=45678
TCP_TOKEN="vifast-tcp-ok"
TCP_TOKEN_LEN=${#TCP_TOKEN}
TCP_SEND_LEN=15 # `vifast-tcp-send`
TCP_CONNECT_LINE="^# net: tcp connect 10\\.0\\.2\\.2:${TCP_PORT} ok rtt=[0-9]+ state=established$"
TCP_RECV_LINE="^# net: tcp recv=${TCP_TOKEN_LEN} first=${TCP_TOKEN}$"
TCP_DATA_LINE="^# net tcp data: recv=${TCP_TOKEN_LEN} sent=${TCP_SEND_LEN} send=ok close=ok state=(fin-wait-2|time-wait|closing|last-ack|closed) retransmits=[0-9]+ elapsed_ms=[0-9]+ const:timeout_ms=3000$"
DNS_LINE='^# net: dns example\.com a=[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+ ttl=[0-9]+$'
# **KT-294** — dòng `# net dns:` (bằng chứng **thử lại** DNS): `attempts=1` (lần đầu được) hoặc
# `attempts=2` (lần đầu timeout, gửi lại **cùng `id`**). `attempts=3` là **retransmit vô hạn** ⇒
# ĐỎ. Ngân sách **mỗi lần thử** vẫn `const:timeout_ms=3000` — không nới ngưỡng.
DNS_REPLY_LINE='^# net dns: name=example\.com id=0x4b55 rcode=0 an=1 reply_len=[0-9]+ rtt_ms=[0-9]+ elapsed_ms=[0-9]+ attempts=(1|2) const:timeout_ms=3000$'

case "${1:-}" in
'' | *[!0-9]*) ;;
*) N_BOOT="$1" ;;
esac

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

die() {
    echo "run-aarch64.sh: $*" >&2
    exit 1
}

mkdir -p "$LOGDIR" 2>/dev/null || true

# ---------------------------------------------------------------- đĩa tạm (KT-273)
# Nội dung **biết trước**: 8 sector 512 B, 17 byte đầu là chữ ký `VIFASTOS-KT273-S0` (8 byte đầu
# khớp `virtio_blk::SECTOR0_MAGIC`). Ảnh in hex 16 byte đầu của sector 0; runner băm lại **cùng**
# 16 byte từ tệp ⇒ "đọc được sector 0" không thể là đọc ra số 0 rồi tự khen.
DISK="$LOGDIR/disk0.raw"
make_disk() {
    dd if=/dev/zero of="$DISK" bs=512 count=8 2>/dev/null || return 1
    printf 'VIFASTOS-KT273-S0' | dd of="$DISK" bs=1 seek=0 conv=notrunc 2>/dev/null || return 1
    return 0
}
make_disk || die "không tạo được đĩa tạm $DISK"
WANT_SECTOR0_HEX="$(od -An -tx1 -N16 "$DISK" 2>/dev/null | tr -d ' \n')"
[ "${#WANT_SECTOR0_HEX}" -eq 32 ] || die "không đọc được 16 byte đầu của $DISK"

# ---------------------------------------------------------------- objdump (nhiều nguồn)
# macOS không có `llvm-objdump` trong PATH mặc định; `llvm-tools` của rustup thì có. Thử lần lượt
# và **fail-closed** nếu không có cái nào (không bỏ qua kiểm ELF).
find_objdump() {
    local c
    for c in llvm-objdump rust-objdump; do
        if command -v "$c" >/dev/null 2>&1; then
            command -v "$c"
            return 0
        fi
    done
    local sysroot host
    sysroot="$(rustc --print sysroot 2>/dev/null || true)"
    host="$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')"
    if [ -n "$sysroot" ] && [ -n "$host" ] && [ -x "$sysroot/lib/rustlib/$host/bin/llvm-objdump" ]; then
        echo "$sysroot/lib/rustlib/$host/bin/llvm-objdump"
        return 0
    fi
    if command -v xcrun >/dev/null 2>&1; then
        c="$(xcrun --find llvm-objdump 2>/dev/null || true)"
        if [ -n "$c" ] && [ -x "$c" ]; then
            echo "$c"
            return 0
        fi
    fi
    if command -v objdump >/dev/null 2>&1; then
        command -v objdump
        return 0
    fi
    return 1
}

# ---------------------------------------------------------------- kiểm ELF (T-objdump)
# `check_elf <image>`: 0 ⇔ mọi kiểm ELF xanh. In lý do đỏ ra stdout (dòng `#`).
check_elf() {
    local img="$1" bad=0
    local hdr text_vma vec_vma vec_type base addr want i

    hdr="$("$OBJDUMP" -h "$img" 2>/dev/null)" || {
        echo "# không đọc được header ELF bằng $OBJDUMP"
        return 1
    }
    text_vma="$(printf '%s\n' "$hdr" | awk '$2==".text"{print $4}')"
    vec_vma="$(printf '%s\n' "$hdr" | awk '$2==".text.vectors"{print $4}')"
    vec_type="$(printf '%s\n' "$hdr" | awk '$2==".text.vectors"{print $5}')"

    if [ -z "$text_vma" ]; then
        echo "# không thấy section .text"
        bad=1
    elif [ "$((0x$text_vma))" != "$((EXPECT_TEXT_VMA))" ]; then
        echo "# .text VMA=$text_vma, cần $EXPECT_TEXT_VMA"
        bad=1
    fi

    # Bất biến B3.2: bảng vector **không** nằm trong `.bss` (`.bss` là NOLOAD ⇒ không có trong ảnh).
    if [ -z "$vec_vma" ]; then
        echo "# không thấy section .text.vectors (bảng vector phải ở section riêng)"
        bad=1
    else
        if [ "$vec_type" != "TEXT" ]; then
            echo "# .text.vectors type=$vec_type, cần TEXT (KHÔNG được BSS)"
            bad=1
        fi
        if [ "$((0x$vec_vma % 0x800))" -ne 0 ]; then
            echo "# .text.vectors VMA=$vec_vma không căn 0x800 (VBAR_EL1 chỉ giữ bit [63:11])"
            bad=1
        fi
    fi

    # 16 stub đúng `base + i*0x80` — kiểm phía **asm**, không chỉ phía Rust.
    local syms
    syms="$("$OBJDUMP" -t "$img" 2>/dev/null)"
    base="$(printf '%s\n' "$syms" | awk '$NF=="__vector_base"{print $1}')"
    if [ -z "$base" ]; then
        echo "# không thấy symbol __vector_base"
        bad=1
    else
        i=0
        while [ "$i" -lt 16 ]; do
            addr="$(printf '%s\n' "$syms" | awk -v s="__vec_$i" '$NF==s{print $1}')"
            want="$(printf '%016x' $((0x$base + i * 0x80)))"
            if [ "$addr" != "$want" ]; then
                echo "# __vec_$i=$addr, cần $want (base=$base)"
                bad=1
            fi
            i=$((i + 1))
        done
    fi
    return "$bad"
}

# ---------------------------------------------------------------- kiểm dòng `# smp:` (KT-259/262)
# Các dòng **bằng chứng hart** phải có **đúng một lần** mỗi dòng. Đây là phần "mỗi hart in hart id
# của mình" của tiêu chí KT-259/262: TAP nói phép đo đúng, các dòng này nói **hart nào** đã in —
# một ảnh chỉ chạy hart 0 mà in `ok 10` vẫn bị bắt ở đây.
#
# So **chuỗi con** (`grep -Fc`, không phải `-Fxc`): dòng IPI của ảnh là
# `# smp: ipi recv=1 intid=0` — mang thêm INTID đọc từ `ICC_IAR1_EL1` sau đúng tiền tố mà KT-259
# yêu cầu. Vẫn đòi **đúng một** dòng khớp, nên in lặp (ca âm g) vẫn đỏ.
#
# **KT-262**: thêm `online`/`alive` cho hart 2 và hart 3 — bốn hart phải cùng in, không chỉ hai.
check_smp() {
    local log="$1" bad=0 line n
    for line in \
        "# smp: hart=1 online" \
        "# smp: hart=2 online" \
        "# smp: hart=3 online" \
        "# smp: ipi recv=1" \
        "# smp: hart=0 alive" \
        "# smp: hart=1 alive" \
        "# smp: hart=2 alive" \
        "# smp: hart=3 alive"; do
        n="$(grep -Fc "$line" "$log" || true)"
        if [ "${n:-0}" -ne 1 ]; then
            echo "# dòng '$line' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        fi
    done
    # **KT-338c** (D2-244 tầng 1) — dòng bằng chứng **hộp thư IPI bitmask dùng chung**:
    #   `# ipi: mailbox posted=<n> coalesced=<n> sent=<n> drained=<n> empty=<n> sgi=0 target=1`
    # Đúng **một** lần, và `drained >= 1`, `empty = 0`: nếu SGI tới mà bitmask rỗng thì wake đã
    # bị mất (đúng bẫy KT-336c) — TAP 11 vẫn có thể `ok` nếu chỉ đếm IPI, nên phép kiểm này mới là
    # thứ **đo** tầng hộp thư IPI.
    n="$(grep -cE '^# ipi: mailbox posted=[0-9]+ coalesced=[0-9]+ sent=[0-9]+ drained=[0-9]+ empty=[0-9]+ sgi=0 target=1$' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# ipi: mailbox …' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    line="$(grep -E '^# ipi: mailbox ' "$log" || true)"
    dband="$(printf '%s\n' "$line" | sed -n 's/.* drained=\([0-9][0-9]*\).*/\1/p')"
    dbempty="$(printf '%s\n' "$line" | sed -n 's/.* empty=\([0-9][0-9]*\).*/\1/p')"
    if [ -z "$dband" ] || [ "$dband" -lt 1 ]; then
        echo "# '# ipi: mailbox' có drained=${dband:-none} — hart đích chưa drain bitmask (hộp thư chưa nối)"
        bad=1
    fi
    if [ -z "$dbempty" ] || [ "$dbempty" -ne 0 ]; then
        echo "# '# ipi: mailbox' có empty=${dbempty:-none} — SGI tới mà bitmask rỗng (mất đánh thức)"
        bad=1
    fi
    return "$bad"
}

# ---------------------------------------------------------------- kiểm dòng `# tlbi:` (KT-262)
# Ba dòng **bằng chứng shootdown** — đây là phần "TLB của hart khác đã bị xoá" của B3.6:
#
#   `# tlbi: hart=1 read1=… match=1`   hart 1 **đã nạp** entry vào TLB (nếu không, "fault" sau
#                                      `unmap` không chứng minh được gì);
#   `# tlbi: shootdown … acks=N expected=N ok=1`
#                                      **mọi** hart phụ đã ack (N = `EXPECT_SMP - 1`);
#   `# tlbi: hart=1 read2 … fault=1`   hart 1 **fault** sau `unmap` ⇒ TLB của nó đã sạch.
#
# Đòi **đúng một** dòng mỗi loại: in lặp nghĩa là shootdown chạy hai lần (hoặc log bịa).
check_tlbi() {
    local log="$1" bad=0 n line
    for line in \
        "# tlbi: hart=1 read1=" \
        "# tlbi: shootdown " \
        "# tlbi: hart=1 read2 "; do
        n="$(grep -Fc "$line" "$log" || true)"
        if [ "${n:-0}" -ne 1 ]; then
            echo "# dòng '$line' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        fi
    done
    # Nội dung từng dòng: `match=1` (hart 1 đã nạp TLB), `acks=N expected=N ok=1` (đủ ack),
    # `fault=1` (hart 1 fault sau unmap). So **chuỗi con** nên thêm trường vẫn khớp.
    grep -F "# tlbi: hart=1 read1=" "$log" | grep -Fq "match=1" || {
        echo "# dòng read1 của hart 1 không có 'match=1' (hart 1 chưa nạp TLB)"
        bad=1
    }
    # Ba trường của dòng shootdown kiểm **riêng** (thứ tự in là `expected=` rồi `acks=`, và
    # đổi thứ tự in không được làm bộ kiểm mù).
    local sd
    sd="$(grep -F "# tlbi: shootdown " "$log" || true)"
    case "$sd" in
        *"expected=$EXPECT_ACKS"*) ;;
        *)
            echo "# dòng shootdown không có 'expected=$EXPECT_ACKS'"
            bad=1
            ;;
    esac
    case "$sd" in
        *"acks=$EXPECT_ACKS"*) ;;
        *)
            echo "# dòng shootdown không có 'acks=$EXPECT_ACKS' (thiếu ack)"
            bad=1
            ;;
    esac
    case "$sd" in
        *"ok=1"*) ;;
        *)
            echo "# dòng shootdown không có 'ok=1'"
            bad=1
            ;;
    esac
    grep -F "# tlbi: hart=1 read2 " "$log" | grep -Fq "fault=1" || {
        echo "# dòng read2 của hart 1 không có 'fault=1' (TLB của hart 1 chưa bị xoá)"
        bad=1
    }
    return "$bad"
}

# ---------------------------------------------------------------- kiểm hash ảnh (KT-265 / D2-80)
# `check_digest <log>`: 0 ⇔ log có **đúng một** `# conf image: build=<16 hex> text=<len>` và cả
# hai trường khớp ELF đang chạy (`WANT_BUILD`/`WANT_TEXT` do `image_digest.py` băm từ tệp).
# Đây là bất biến **neo ảnh**: sha256 tệp không chứng minh được đoạn mã **đang chạy** là đoạn mã
# trong tệp; dòng kernel tự in thì chứng minh được.
check_digest() {
    local log="$1" bad=0 n got_build got_text
    n="$(grep -cE '^# conf image: build=[0-9a-f]{16} text=[0-9]+$' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# conf image:' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    got_build="$(sed -n 's/^# conf image: build=\([0-9a-f]*\) text=.*/\1/p' "$log" | head -1)"
    got_text="$(sed -n 's/^# conf image: build=.* text=\([0-9]*\)$/\1/p' "$log" | head -1)"
    if [ "$got_build" != "$WANT_BUILD" ]; then
        echo "# image-build=${got_build:-none} lệch ELF ($WANT_BUILD)"
        bad=1
    fi
    if [ "$got_text" != "$WANT_TEXT" ]; then
        echo "# image-text=${got_text:-none} lệch ELF ($WANT_TEXT)"
        bad=1
    fi
    return "$bad"
}

# ---------------------------------------------------------------- kiểm dòng `# umode:` (KT-269)
# **Bằng chứng U-mode**: đúng **một** dòng `# umode: el0 ran svc=<n> ret=<v>` — nó chỉ được in
# khi handler `svc` **thật sự** đọc được số hiệu syscall từ `x8` của EL0 và trả kết quả qua
# `x1` (quy ước `[calling.aarch64]`). Ảnh chỉ chạy EL1 (không `eret` xuống EL0) sẽ **không** có
# dòng này ⇒ ĐỎ, dù TAP 1..15 vẫn xanh.
check_umode() {
    local log="$1" bad=0 n
    n="$(grep -cE '^# umode: el0 ran svc=[0-9]+ ret=[0-9]+$' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# umode: el0 ran svc=<n> ret=<v>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    # `svc #0` ⇒ imm16 = 0; `ret` phải > 0 (DebugPutStr trả độ dài chuỗi đã in).
    grep -Eq '^# umode: el0 ran svc=0 ret=[1-9][0-9]*$' "$log" || {
        echo "# dòng '# umode: el0 ran svc=0 ret=<len>' không đúng (imm16 phải 0, ret > 0)"
        bad=1
    }
    return "$bad"
}

# ---------------------------------------------------------------- kiểm dòng `# pci:` (KT-273)
# Ba dòng **bằng chứng PCI** — đây là phần "ECAM map đúng + BAR gán rồi đọc lại" của KT-273:
#
#   `# pci: ecam … class=0x060000 … empty=0xffff …`
#       host bridge **phải** là class `0x060000` (PCI class code "bridge/host") và một slot
#       **trống** phải trả `0xffff`. Hai điều kiện ấy **cùng đúng** chỉ khi ECAM trỏ đúng chỗ và
#       giải mã thật — ECAM trỏ sai thì một trong hai sai.
#   `# pci: virtio-blk found … vendor=0x1af4 … assigned=… readback=…`
#       BAR gán xong phải **đọc lại** đúng địa chỉ đã ghi (nếu không, thiết bị không nhận BAR).
#
# Đòi **đúng một** dòng mỗi loại: in lặp nghĩa là scan chạy hai lần (hoặc log bịa).
check_pci() {
    local log="$1" bad=0 n line assigned readback
    n="$(grep -cE '^# pci: ecam ' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# pci: ecam ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    line="$(grep -E '^# pci: ecam ' "$log" || true)"
    case "$line" in
        *"class=0x060000"*) ;;
        *)
            echo "# dòng '# pci: ecam' không có 'class=0x060000' (host bridge class)"
            bad=1
            ;;
    esac
    case "$line" in
        *"empty=0xffff"*) ;;
        *)
            echo "# dòng '# pci: ecam' không có 'empty=0xffff' (slot trống phải trả 0xffff)"
            bad=1
            ;;
    esac
    # **KT-302**: phải quét **đủ 256 bus** (ECAM 256 MiB map bằng block 2 MiB). `buses=16` là
    # đúng hành vi cũ (KT-273) — runner phải ĐỎ, không được coi là "vẫn chạy".
    case "$line" in
        *"buses=256"*) ;;
        *)
            echo "# dòng '# pci: ecam' không có 'buses=256' (KT-302: phải quét đủ 256 bus)"
            bad=1
            ;;
    esac
    # **KT-302**: bằng chứng **đo được** rằng ECAM map bằng **block descriptor 2 MiB ở mức 2**
    # (128 block, một bảng mức 2) — không phải 128 bảng mức 3. Dòng `# pci: map` do `main.rs` in
    # từ kết quả `walk_steps` trên bảng trang thật, không phải lời khai.
    n="$(grep -cE '^# pci: map ' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# pci: map ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    line="$(grep -E '^# pci: map ' "$log" || true)"
    case "$line" in
        *"ecam_blocks=128"*) ;;
        *)
            echo "# dòng '# pci: map' không có 'ecam_blocks=128' (256 MiB / 2 MiB)"
            bad=1
            ;;
    esac
    case "$line" in
        *"ecam_block_level=2"*) ;;
        *)
            echo "# dòng '# pci: map' không có 'ecam_block_level=2' (block 2 MiB ở mức 2)"
            bad=1
            ;;
    esac
    case "$line" in
        *"ecam_block_ok=1"*) ;;
        *)
            echo "# dòng '# pci: map' không có 'ecam_block_ok=1' (walk thấy block đúng PA)"
            bad=1
            ;;
    esac
    # **KT-305**: `translate`/`unmap` phải **hiểu** block descriptor — dòng `# pci: unmap` do
    # `ecam.rs` in từ phép đo trên bảng trang **thật**: `translate` trả PA + `level=2`, `unmap`
    # **chia nhỏ** block (trang kế cận còn map), map lại được. `blocks_split=0` (xoá cả block)
    # ⇒ ĐỎ — đó là đột biến (i) của KT-305.
    n="$(grep -cE '^# pci: unmap ' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# pci: unmap ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    line="$(grep -E '^# pci: unmap ' "$log" || true)"
    for want in "level=2" "translate_ok=1" "split=1" "neighbor_ok=1" "unmapped_ok=1" "remap_ok=1" "blocks_split=1"; do
        case "$line" in
            *"$want"*) ;;
            *)
                echo "# dòng '# pci: unmap' không có '$want' (KT-305)"
                bad=1
                ;;
        esac
    done
    n="$(grep -cE '^# pci: virtio-blk found ' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# pci: virtio-blk found ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    line="$(grep -E '^# pci: virtio-blk found ' "$log" || true)"
    case "$line" in
        *"vendor=0x1af4"*) ;;
        *)
            echo "# dòng virtio-blk không có 'vendor=0x1af4' (vendor của virtio)"
            bad=1
            ;;
    esac
    assigned="$(printf '%s\n' "$line" | sed -n 's/.* assigned=\(0x[0-9a-f]*\).*/\1/p')"
    readback="$(printf '%s\n' "$line" | sed -n 's/.* readback=\(0x[0-9a-f]*\).*/\1/p')"
    if [ -z "$assigned" ] || [ "$assigned" != "$readback" ]; then
        echo "# BAR đọc lại (${readback:-none}) khác địa chỉ đã gán (${assigned:-none})"
        bad=1
    fi
    return "$bad"
}

# ---------------------------------------------------------------- kiểm dòng `# blk:` (KT-273)
# `# blk: sector0 hex=<32 hex> … magic=1` — hex phải khớp **16 byte đầu của đĩa tạm** (runner
# băm lại từ tệp bằng `od`) và `magic=1` chỉ khi 8 byte đầu khớp `VIFASTOS`. Nhờ vậy "đọc được
# sector 0" không thể là đọc ra số 0 rồi tự khen.
check_blk() {
    local log="$1" bad=0 n line
    n="$(grep -cE '^# blk: sector0 hex=[0-9a-f]{32} ' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# blk: sector0 hex=…' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    line="$(grep -E '^# blk: sector0 ' "$log" || true)"
    case "$line" in
        *"hex=$WANT_SECTOR0_HEX"*) ;;
        *)
            echo "# sector0 hex không khớp 16 byte đầu của đĩa tạm ($WANT_SECTOR0_HEX)"
            bad=1
            ;;
    esac
    case "$line" in
        *"magic=1"*) ;;
        *)
            echo "# sector0 không có 'magic=1' (chữ ký VIFASTOS không khớp)"
            bad=1
            ;;
    esac
    return "$bad"
}

# ---------------------------------------------------------------- kiểm dòng `# net:` (KT-275)
# Bốn dòng **bằng chứng mạng** — đây là phần "virtio-net + DHCP + ARP + ICMP thật" của KT-275:
#
#   `# net: device … vendor=0x1af4 … mac=<6 octet> … assigned=… readback=…`
#       MAC **đọc từ `virtio_net_config`** (không hardcode) và BAR gán xong phải **đọc lại**
#       đúng địa chỉ đã ghi.
#   `# net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2`
#       DHCP **thật** của slirp — cùng khuôn `net.sh` riscv64 (KT-246), nên hai arch không lệch
#       nhau về định nghĩa "lấy được địa chỉ".
#   `# net: arp gw=<mac>` — ARP reply **thật**; MAC toàn 0/broadcast là giá trị vô nghĩa.
#   `# net: ping 10.0.2.2 ok n=3 rtt=<ms>` — ICMP echo **thật** qua gateway.
#
# Đòi **đúng một** dòng mỗi loại: in lặp nghĩa là bước mạng chạy hai lần (hoặc log bịa).
check_net() {
    local log="$1" bad=0 n line assigned readback
    n="$(grep -cE '^# net: device ' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# net: device ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    line="$(grep -E '^# net: device ' "$log" || true)"
    case "$line" in
        *"vendor=0x1af4"*) ;;
        *)
            echo "# dòng '# net: device' không có 'vendor=0x1af4' (vendor của virtio)"
            bad=1
            ;;
    esac
    # MAC phải là **6 octet hex** — MAC rỗng/toàn 0 là dấu hiệu đọc sai `virtio_net_config`.
    printf '%s\n' "$line" | grep -Eq 'mac=([0-9a-f]{2}:){5}[0-9a-f]{2}( |$)' || {
        echo "# dòng '# net: device' không có 'mac=<6 octet>'"
        bad=1
    }
    printf '%s\n' "$line" | grep -Eq 'mac=(00:){5}00( |$)' && {
        echo "# MAC toàn 0 — không phải MAC đọc từ thiết bị"
        bad=1
    }
    assigned="$(printf '%s\n' "$line" | sed -n 's/.* assigned=\(0x[0-9a-f]*\).*/\1/p')"
    readback="$(printf '%s\n' "$line" | sed -n 's/.* readback=\(0x[0-9a-f]*\).*/\1/p')"
    if [ -z "$assigned" ] || [ "$assigned" != "$readback" ]; then
        echo "# BAR mạng đọc lại (${readback:-none}) khác địa chỉ đã gán (${assigned:-none})"
        bad=1
    fi
    n="$(grep -cE '^# net: dhcp ip=10\.0\.2\.15 gw=10\.0\.2\.2 mask=255\.255\.255\.0 server=10\.0\.2\.2$' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    n="$(grep -cE '^# net: arp gw=[0-9a-f]{2}(:[0-9a-f]{2}){5}$' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# net: arp gw=<mac>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    # MAC toàn 0 / broadcast là giá trị **vô nghĩa** cho một host — driver từ chối, nên log có nó
    # nghĩa là phép kiểm của driver bị bỏ (cùng bất biến `arp_parse_reply`).
    grep -E '^# net: arp gw=' "$log" | grep -Eq 'gw=(00:){5}00$' && {
        echo "# ARP trả MAC toàn 0 — không phải reply hợp lệ"
        bad=1
    }
    grep -E '^# net: arp gw=' "$log" | grep -Eq 'gw=(ff:){5}ff$' && {
        echo "# ARP trả MAC broadcast — không phải reply hợp lệ"
        bad=1
    }
    n="$(grep -cE '^# net: ping 10\.0\.2\.2 ok n=3 rtt=[0-9]+$' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# net: ping 10.0.2.2 ok n=3 rtt=<ms>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    # **KT-289** — TCP client + DNS. Ba dòng bằng chứng, mỗi dòng **đúng một** lần:
    #   `# net: tcp connect 10.0.2.2:45678 ok rtt=<ms> state=established` — bắt tay ba bước thật;
    #   `# net: tcp recv=13 first=vifast-tcp-ok` — nhận **đúng** chuỗi server gửi;
    #   `# net tcp data: recv=13 sent=15 send=ok close=ok …` — gửi lại + đóng FIN;
    #   `# net: dns example.com a=<ip> ttl=<n>` — DNS **thật** của slirp.
    # Dòng `# net: no tcp`/`# net: no dns` là dấu hiệu **thất bại** — có nó là ĐỎ.
    n="$(grep -cE "$TCP_CONNECT_LINE" "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# net: tcp connect 10.0.2.2:$TCP_PORT ok rtt=<ms> state=established' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    n="$(grep -cE "$TCP_RECV_LINE" "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# net: tcp recv=$TCP_TOKEN_LEN first=$TCP_TOKEN' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    n="$(grep -cE "$TCP_DATA_LINE" "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# net tcp data:' (recv=$TCP_TOKEN_LEN sent=$TCP_SEND_LEN send=ok close=ok) xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    n="$(grep -cE "$DNS_LINE" "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# net: dns example.com a=<ip> ttl=<n>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    # **KT-294** — dòng `# net dns:` phải có `attempts=1|2`: chứng minh ảnh **có** chính sách thử
    # lại và **không** retransmit vô hạn (`attempts=3` ⇒ ĐỎ). `attempts=2` = lần đầu timeout.
    n="$(grep -cE "$DNS_REPLY_LINE" "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# net dns: … attempts=(1|2) const:timeout_ms=3000' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    for want in '# net: no tcp' '# net: no dns'; do
        n="$(grep -Fc "$want" "$log" || true)"
        if [ "${n:-0}" -ne 0 ]; then
            echo "# log có '$want' (bước TCP/DNS thất bại)"
            bad=1
        fi
    done
    [ "$bad" -eq 0 ]
}

# ---------------------------------------------------------------- chụp màn hình qua monitor (KT-279)
# `screendump <tệp.ppm>`: nối **một** phiên HMP, đọc banner tới `(qemu) `, gửi `screendump`, đọc
# prompt. Trả 0 = đã gửi; 1 = không nối được / monitor báo lỗi. Cùng khuôn `gpu.sh` (KT-263).
screendump() {
    local path="$1"
    "$PYTHON" - "$SOCK" "$path" <<'PY'
import socket
import sys

sock, path = sys.argv[1], sys.argv[2]
try:
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(5.0)
    s.connect(sock)
except OSError as exc:
    print(f"# run-aarch64.sh: không nối được monitor {sock}: {exc}", file=sys.stderr)
    sys.exit(1)


def read_prompt():
    buf = b""
    try:
        while b"(qemu)" not in buf:
            chunk = s.recv(4096)
            if not chunk:
                break
            buf += chunk
    except socket.timeout:
        pass
    return buf


read_prompt()
s.sendall(("screendump " + path + "\n").encode())
out = read_prompt()
s.close()
if b"Error" in out or b"error" in out:
    print(f"# run-aarch64.sh: monitor báo lỗi: {out!r}", file=sys.stderr)
    sys.exit(1)
sys.exit(0)
PY
}

# `send_keys`: nối **một** phiên HMP, đọc banner tới `(qemu) `, gửi `sendkey a`, đọc prompt.
# Trả 0 = đã gửi; 1 = không nối được / monitor không trả prompt. Cùng kỹ thuật `input.sh` riscv64
# (KT-255) — **không** `sleep` mù trước khi gửi: mốc là dòng `# input: ready` của khách.
send_keys() {
    "$PYTHON" - "$SOCK" <<'PY'
import socket
import sys
import time

sock = sys.argv[1]
try:
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(5.0)
    s.connect(sock)
except OSError as exc:
    print(f"# run-aarch64.sh: không nối được monitor {sock}: {exc}", file=sys.stderr)
    sys.exit(1)


def read_prompt():
    buf = b""
    try:
        while b"(qemu)" not in buf:
            chunk = s.recv(4096)
            if not chunk:
                break
            buf += chunk
    except socket.timeout:
        pass
    return buf


read_prompt()
s.sendall(b"sendkey a\n")
read_prompt()
time.sleep(0.05)
s.close()
sys.exit(0)
PY
}

# ---------------------------------------------------------------- TCP server trên host (KT-289)
# Vì sao `python3` chứ không `socat`/`nc`: hai công cụ đó **có thể không có** trên máy đo; python3
# là thứ runner đã đòi cho monitor/`gpu_ppm.py`. Server ghi log từng mốc
# (`listen`/`accept`/`sent`/`recv`) để runner có bằng chứng **hai chiều** khách ↔ host, không chỉ
# tin log của khách. Cùng khuôn `net.sh` riscv64 (KT-268) — **không** có bản thứ hai lệch hành vi.
TCP_SERVER_PID=""

write_tcp_server() {
    cat >"$LOGDIR/kt289-tcp-server.py" <<'PY'
import socket, sys

port = int(sys.argv[1])
token = sys.argv[2].encode()
log = sys.argv[3]


def note(line):
    with open(log, "a") as handle:
        handle.write(line + "\n")


srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
try:
    srv.bind(("127.0.0.1", port))
except OSError as error:
    note("bind_err=%s" % error)
    sys.exit(2)
srv.listen(1)
note("listen port=%d" % port)
srv.settimeout(60.0)
try:
    conn, addr = srv.accept()
except socket.timeout:
    note("accept_timeout")
    sys.exit(3)
note("accept peer=%s:%d" % addr)
conn.sendall(token)
note("sent=%d" % len(token))
conn.settimeout(5.0)
total = 0
try:
    while True:
        data = conn.recv(4096)
        if not data:
            break
        total += len(data)
except socket.timeout:
    note("recv_timeout")
note("recv=%d" % total)
conn.close()
srv.close()
PY
}

# `start_tcp_server <log>`: dựng server, chờ dòng `listen` (tối đa 5 s). 0 = sẵn sàng.
start_tcp_server() {
    local srv_log="$1" i
    command -v python3 >/dev/null 2>&1 || {
        echo "run-aarch64.sh: không có python3 để dựng TCP server" >&2
        return 1
    }
    write_tcp_server
    rm -f "$srv_log"
    python3 "$LOGDIR/kt289-tcp-server.py" "$TCP_PORT" "$TCP_TOKEN" "$srv_log" &
    TCP_SERVER_PID=$!
    for i in $(seq 1 50); do
        if grep -q '^listen ' "$srv_log" 2>/dev/null; then
            return 0
        fi
        if grep -q '^bind_err=' "$srv_log" 2>/dev/null; then
            echo "run-aarch64.sh: TCP server không bind được 127.0.0.1:$TCP_PORT — port đang bận?" >&2
            cat "$srv_log" >&2
            return 1
        fi
        sleep 0.1
    done
    echo "run-aarch64.sh: TCP server không lắng nghe sau 5 s" >&2
    return 1
}

# `wait_tcp_server <log>`: chờ server xong (tối đa 20 s) và đòi đủ ba mốc bằng chứng.
wait_tcp_server() {
    local srv_log="$1" i
    for i in $(seq 1 200); do
        if grep -qE '^(recv=|accept_timeout|bind_err)' "$srv_log" 2>/dev/null; then
            break
        fi
        sleep 0.1
    done
    if [ -n "$TCP_SERVER_PID" ]; then
        if kill -0 "$TCP_SERVER_PID" 2>/dev/null; then
            kill "$TCP_SERVER_PID" 2>/dev/null || true
            echo "run-aarch64.sh: server TCP vẫn chạy sau 20 s — đã kill" >&2
            TCP_SERVER_PID=""
            return 1
        fi
        wait "$TCP_SERVER_PID" 2>/dev/null || true
        TCP_SERVER_PID=""
    fi
    grep -q '^accept ' "$srv_log" || {
        echo "run-aarch64.sh: server TCP không nhận kết nối từ khách" >&2
        return 1
    }
    grep -qx "sent=$TCP_TOKEN_LEN" "$srv_log" || {
        echo "run-aarch64.sh: server TCP không gửi đủ $TCP_TOKEN_LEN byte" >&2
        return 1
    }
    grep -qx "recv=$TCP_SEND_LEN" "$srv_log" || {
        echo "run-aarch64.sh: server TCP không nhận đúng $TCP_SEND_LEN byte từ khách" >&2
        return 1
    }
    return 0
}

stop_tcp_server() {
    if [ -n "$TCP_SERVER_PID" ]; then
        kill "$TCP_SERVER_PID" 2>/dev/null || true
        wait "$TCP_SERVER_PID" 2>/dev/null || true
        TCP_SERVER_PID=""
    fi
}

# `boot_full <log> <ppm>`: khởi động QEMU **có màn hình + bàn phím + monitor**, chờ marker của
# **khách** (`# gpu: console done lines=3`), chụp PPM, chờ `# input: ready` rồi bơm `sendkey a`,
# rồi chờ QEMU tự thoát trong ngân sách.
#
# Vì sao không dùng `qg_boot`: `qg_boot` chờ QEMU thoát **rồi** mới trả về, mà ảnh `halt` ngay sau
# marker ⇒ monitor socket biến mất trước khi runner kịp nối (đo được bên riscv64). Ảnh giữ khung
# hình `gpu::GPU_HOLD_MS` = 500 ms để runner kịp; runner **không** `sleep` mù — mốc là marker của
# khách, không phải đồng hồ của host.
#
# **KT-283**: cùng một phiên monitor phục vụ **hai** việc (screendump của GPU + `sendkey` của bàn
# phím) — không mở socket thứ hai, và **mọi** boot (kể cả boot 2..N) đi qua đây để "ảnh nào" không
# lệch giữa boot 1 và boot N.
#
# Trả 0 = QEMU tự thoát; 124 = quá hạn (đã kill cả group); 125 = không khởi động được.
boot_full() {
    local out="$1" ppm="$2"
    rm -f "$SOCK" "$ppm" 2>/dev/null
    qg_tmp "$SOCK"
    qg_start "$out" "$TIMEOUT_DS" "$QEMU" -M virt,gic-version=3 -cpu cortex-a72 -m 128M \
        -nographic -smp "$EXPECT_SMP" -kernel "$IMAGE" -no-reboot -nic none \
        -netdev user,id=n0 -device virtio-net-pci,netdev=n0 \
        -drive "file=$DISK,if=none,id=d0,format=raw,snapshot=on" \
        -device virtio-blk-pci,drive=d0 -device virtio-gpu-pci \
        -device virtio-keyboard-pci \
        -monitor "unix:$SOCK,server=on,wait=off" || return 125
    local pid="$QG_LAST_PID" waited="${QG_START_SPENT:-0}" i=0
    # 1) chờ marker GPU của **khách** (không sleep mù): ảnh in sau lô flush console.
    while [ "$i" -lt "$READY_DS" ] && kill -0 "$pid" 2>/dev/null; do
        grep -aq '^# gpu: console done lines=3$' "$out" 2>/dev/null && break
        sleep 0.05
        i=$((i + 1))
    done
    if grep -aq '^# gpu: console done lines=3$' "$out" 2>/dev/null; then
        screendump "$ppm" || true
    fi
    # 2) **KT-283**: chờ `# input: ready` (ảnh đã arm 8 đệm) rồi bơm `sendkey a`.
    i=0
    while [ "$i" -lt "$READY_DS" ] && kill -0 "$pid" 2>/dev/null; do
        grep -aq '^# input: ready$' "$out" 2>/dev/null && break
        sleep 0.05
        i=$((i + 1))
    done
    if grep -aq '^# input: ready$' "$out" 2>/dev/null; then
        # Gửi hỏng ⇒ TAP 32 đỏ ⇒ `check_log` bắt; không nuốt lỗi thành "xanh".
        send_keys || true
    fi
    # 3) chờ QEMU tự thoát trong phần ngân sách còn lại (cùng công thức `qg_boot`).
    local t0=$SECONDS tmax=$(((TIMEOUT_DS / 10) * 2 + 5))
    while kill -0 "$pid" 2>/dev/null && [ "$waited" -lt "$TIMEOUT_DS" ] &&
        [ $((SECONDS - t0)) -lt "$tmax" ]; do
        sleep 0.1
        waited=$((waited + 1))
    done
    if kill -0 "$pid" 2>/dev/null; then
        qg_kill_group "$pid"
        wait "$pid" 2>/dev/null
        return 124
    fi
    wait "$pid" 2>/dev/null
    return 0
}

# ---------------------------------------------------------------- kiểm dòng `# gpu:` (KT-279)
# Mười một dòng **bằng chứng GPU** — đây là phần "virtio-gpu 2D + console trên framebuffer" của
# KT-279. Mỗi dòng đòi **đúng một** lần (in lặp = bước GPU chạy hai lần, hoặc log bịa):
#
#   `# gpu pci: … vendor=0x1af4 device=0x1050 … assigned=0x10800000 readback=0x10800000`
#       BAR gán xong phải **đọc lại** đúng, và đúng leaf **riêng** của GPU (không đụng blk/net).
#   `# gpu dev: … version1=1 device_status=0x0f num_scanouts=1`
#       feature VERSION_1 hai chiều + `device_status` còn DRIVER_OK + thiết bị báo **có** scanout.
#   `# gpu fb: … identity=1 bytes=1228800 pages=300`
#       framebuffer **ngoài ảnh**, map identity (PA **đọc lại từ bảng trang**) — nếu framebuffer
#       nằm trong `.bss` thì `bytes` sẽ không phải 1.228.800 (trần I7 262.144 B).
#   `# gpu scanout: … bogus=InvalidResourceId`
#       ca âm đối chứng: thiết bị **kiểm** `resource_id`.
#   `# gpu console: … chars=29 newlines=3 scrolls=0 dirty=8,8,88,24 flushes=1 commands=2
#        matched=29/29 text=ViFastOs|gpu console|0123456789`
#       flush **một lô** = đúng **hai** lệnh cho cả khối (flush từng ký tự sẽ là 58).
#   `# gpu stats: … errors=0 timeouts=0 id_mismatch=0 rejects=1 … fb_clean_lines=<n>`
#       đường đi sạch + cache maintenance framebuffer **đã chạy** (`n > 0`).
check_gpu() {
    local log="$1" bad=0 n line assigned readback lines bytes ops
    for want in \
        '^# gpu pci: ' \
        '^# gpu dev: ' \
        '^# gpu dma: ' \
        '^# gpu fb: ' \
        '^# gpu display: ' \
        '^# gpu resource: ' \
        '^# gpu scanout: ' \
        '^# gpu draw: ' \
        '^# gpu console: ' \
        '^# gpu stats: '; do
        n="$(grep -cE "$want" "$log" || true)"
        if [ "${n:-0}" -ne 1 ]; then
            echo "# dòng khớp '$want' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        fi
    done
    # Marker của khách — runner chụp PPM **sau** dòng này, nên nó phải có đúng một lần.
    for want in '^# gpu: drawn$' '^# gpu: console done lines=3$'; do
        n="$(grep -cE "$want" "$log" || true)"
        if [ "${n:-0}" -ne 1 ]; then
            echo "# dòng khớp '$want' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        fi
    done
    line="$(grep -E '^# gpu pci: ' "$log" || true)"
    for want in "vendor=0x1af4" "device=0x1050" "assigned=$GPU_BAR_ASSIGNED"; do
        case "$line" in
            *"$want"*) ;;
            *)
                echo "# dòng '# gpu pci:' không có '$want'"
                bad=1
                ;;
        esac
    done
    assigned="$(printf '%s\n' "$line" | sed -n 's/.* assigned=\(0x[0-9a-f]*\).*/\1/p')"
    readback="$(printf '%s\n' "$line" | sed -n 's/.* readback=\(0x[0-9a-f]*\).*/\1/p')"
    if [ -z "$assigned" ] || [ "$assigned" != "$readback" ]; then
        echo "# BAR GPU đọc lại (${readback:-none}) khác địa chỉ đã gán (${assigned:-none})"
        bad=1
    fi
    line="$(grep -E '^# gpu dev: ' "$log" || true)"
    for want in "version1=1" "device_status=0x0f" "num_scanouts=1"; do
        case "$line" in
            *"$want"*) ;;
            *)
                echo "# dòng '# gpu dev:' không có '$want'"
                bad=1
                ;;
        esac
    done
    line="$(grep -E '^# gpu fb: ' "$log" || true)"
    for want in "identity=1" "bytes=1228800" "pages=300" "pa=0x42000000" "walk=0x42000000"; do
        case "$line" in
            *"$want"*) ;;
            *)
                echo "# dòng '# gpu fb:' không có '$want'"
                bad=1
                ;;
        esac
    done
    line="$(grep -E '^# gpu scanout: ' "$log" || true)"
    case "$line" in
        *"bogus=InvalidResourceId"*) ;;
        *)
            echo "# dòng '# gpu scanout:' không có 'bogus=InvalidResourceId' (thiết bị không kiểm resource_id)"
            bad=1
            ;;
    esac
    line="$(grep -E '^# gpu console: ' "$log" || true)"
    for want in "chars=29" "newlines=3" "scrolls=0" "dirty=8,8,88,24" "flushes=1" "commands=2" \
        "matched=29/29" "text=ViFastOs|gpu console|0123456789"; do
        case "$line" in
            *"$want"*) ;;
            *)
                echo "# dòng '# gpu console:' không có '$want'"
                bad=1
                ;;
        esac
    done
    line="$(grep -E '^# gpu stats: ' "$log" || true)"
    for want in "errors=0" "timeouts=0" "id_mismatch=0" "rejects=1"; do
        case "$line" in
            *"$want"*) ;;
            *)
                echo "# dòng '# gpu stats:' không có '$want'"
                bad=1
                ;;
        esac
    done
    # Cache maintenance framebuffer **đã chạy**: `fb_clean_lines`/`fb_clean_bytes`/`fb_clean_ops`
    # đều dương. Bỏ `dc civac` ⇒ `0` ⇒ ĐỎ (tầng quan sát được trên QEMU TCG).
    lines="$(printf '%s\n' "$line" | sed -n 's/.* fb_clean_lines=\([0-9][0-9]*\).*/\1/p')"
    bytes="$(printf '%s\n' "$line" | sed -n 's/.* fb_clean_bytes=\([0-9][0-9]*\).*/\1/p')"
    ops="$(printf '%s\n' "$line" | sed -n 's/.* fb_clean_ops=\([0-9][0-9]*\).*/\1/p')"
    if [ -z "$lines" ] || [ "$lines" -le 0 ]; then
        echo "# dòng '# gpu stats:' có fb_clean_lines=${lines:-none} — không có lệnh 'dc civac' nào chạy trên framebuffer"
        bad=1
    fi
    if [ -z "$bytes" ] || [ "$bytes" -le 0 ]; then
        echo "# dòng '# gpu stats:' có fb_clean_bytes=${bytes:-none} — vùng framebuffer rỗng"
        bad=1
    fi
    if [ -z "$ops" ] || [ "$ops" -le 0 ]; then
        echo "# dòng '# gpu stats:' có fb_clean_ops=${ops:-none} — chưa lần nào làm sạch framebuffer"
        bad=1
    fi
    return "$bad"
}

# ---------------------------------------------------------------- kiểm ảnh chụp PPM (KT-279)
# `check_ppm <ppm>`: 0 ⇔ `gpu_ppm.py` xanh trên ảnh chụp. Đây là **bằng chứng thật** rằng GPU đã
# vẽ: TAP chỉ nói "lệnh gửi xong", còn PPM đòi **đúng màu tại đúng toạ độ** + **từng ký tự** của
# ba dòng console so bitmap 8×8 đọc từ `v2/lib/vifast-virtio/src/gpu.rs`.
check_ppm() {
    local ppm="$1"
    [ -f "$ppm" ] || {
        echo "# không có ảnh chụp $ppm"
        return 1
    }
    "$PYTHON" "$PPM_CHECK" "$ppm" || return 1
    return 0
}

# ---------------------------------------------------------------- kiểm dòng `# wx:` (KT-277)
# `# wx: text=RX data=RW ok=1 text_bytes=… data_bytes=… guard_pages=… guard_bytes=… … fault_ec=0x25
#        fault_access=Write fault_va=… text_unchanged=1 data_rw=1 …`
#
# Đây là phần "**W^X thật**" của KT-277:
#   - `text=RX`/`data=RW`/`ok=1` — quyền **đọc lại từ bảng trang** (không phải hằng viết tay);
#   - `fault_ec=0x25` + `fault_access=Write` — một lần **ghi** vào `.text` từ EL1 đã sinh data
#     abort quyền; ảnh map `.text` RW sẽ **không** fault ⇒ `ok=0` ⇒ ĐỎ;
#   - `text_unchanged=1` — 8 byte của `.text` **không đổi** sau lần ghi bị từ chối;
#   - `data_rw=1` — ghi vào `.data` thì **thành công** (không phải "RX cả ảnh").
#
# **Guard không map** được kiểm bằng **số trang**: `# mmu: map kernel=… pages=N` phải bằng
# `(text_bytes + data_bytes - guard_bytes) / 4096`. Nếu 8 trang guard bị map, `N` lớn hơn đúng 8
# ⇒ ĐỎ. Đây là phép kiểm QEMU-quan-sát-được cho bất biến "guard **không** map".
check_wx() {
    local log="$1" bad=0 n line want text_bytes data_bytes guard_bytes mmu_line mmu_pages want_pages
    n="$(grep -cE '^# wx: ' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# wx: ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    line="$(grep -E '^# wx: ' "$log" || true)"
    for want in "text=RX" "data=RW" "ok=1" "fault_access=Write" "text_unchanged=1" "data_rw=1"; do
        case "$line" in
            *"$want"*) ;;
            *)
                echo "# dòng '# wx:' không có '$want'"
                bad=1
                ;;
        esac
    done
    # Một lần **ghi** vào `.text` phải là data abort **cùng EL** (`EC=0x25`). Nhận `0x21`
    # (instruction abort) làm phương án dự phòng nhưng vẫn đòi **có** fault — không có fault
    # nghĩa là `.text` ghi được.
    case "$line" in
        *"fault_ec=0x25"* | *"fault_ec=0x21"*) ;;
        *)
            echo "# dòng '# wx:' không có 'fault_ec=0x25'/'0x21' (ghi vào .text KHÔNG fault)"
            bad=1
            ;;
    esac
    text_bytes="$(printf '%s\n' "$line" | sed -n 's/.* text_bytes=\([0-9][0-9]*\).*/\1/p')"
    data_bytes="$(printf '%s\n' "$line" | sed -n 's/.* data_bytes=\([0-9][0-9]*\).*/\1/p')"
    guard_bytes="$(printf '%s\n' "$line" | sed -n 's/.* guard_bytes=\([0-9][0-9]*\).*/\1/p')"
    if [ -z "$text_bytes" ] || [ "$text_bytes" -le 0 ]; then
        echo "# dòng '# wx:' thiếu 'text_bytes=' dương (vùng RX rỗng)"
        bad=1
    fi
    if [ -z "$data_bytes" ] || [ "$data_bytes" -le 0 ]; then
        echo "# dòng '# wx:' thiếu 'data_bytes=' dương (vùng RW rỗng)"
        bad=1
    fi
    if [ -z "$guard_bytes" ] || [ "$guard_bytes" -le 0 ]; then
        echo "# dòng '# wx:' thiếu 'guard_bytes=' dương (không có trang guard nào bị loại)"
        bad=1
    fi
    # **Guard không map**: số trang map vùng ảnh phải bằng (text + data - guard) / 4096.
    mmu_line="$(grep -E '^# mmu: map kernel=' "$log" || true)"
    mmu_pages="$(printf '%s\n' "$mmu_line" | sed -n 's/.* pages=\([0-9][0-9]*\).*/\1/p')"
    if [ -n "$text_bytes" ] && [ -n "$data_bytes" ] && [ -n "$guard_bytes" ]; then
        want_pages=$(((text_bytes + data_bytes - guard_bytes) / 4096))
        if [ -z "$mmu_pages" ]; then
            echo "# thiếu dòng '# mmu: map kernel=… pages=…' để đối chiếu số trang"
            bad=1
        elif [ "$mmu_pages" -ne "$want_pages" ]; then
            echo "# số trang map vùng ảnh ($mmu_pages) khác (text+data-guard)/4096 = $want_pages — guard có thể đã bị map"
            bad=1
        fi
    fi
    return "$bad"
}

# ---------------------------------------------------------------- kiểm dòng `# dma:` (KT-277)
# `# dma: cache-clean ok=1 lines=… bytes=… ops=… inval_lines=… inval_bytes=… inval_ops=… …`
#
# `lines`/`inval_lines` là bộ đếm của **chính vòng lặp phát `dc`** (không phải hằng viết tay):
# bỏ lời gọi `dc civac` ⇒ `lines=0` ⇒ ĐỎ. `lines × line == bytes` chốt luôn phép tính dòng.
#
# **Trung thực về QEMU TCG**: TCG **không** mô hình hoá cache, nên **hiệu ứng** của `dc` không
# quan sát được ở đây — tầng này chỉ chứng minh **lệnh đã chạy** (số dòng + số byte). Bằng chứng
# "địa chỉ từng dòng" nằm ở test host `v2/kernel/vifast-plat-aarch64/tests/cache.rs`.
check_dma() {
    local log="$1" bad=0 n line lines bytes line_bytes inval_lines
    n="$(grep -cE '^# dma: ' "$log" || true)"
    if [ "${n:-0}" -ne 1 ]; then
        echo "# dòng '# dma: ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    fi
    line="$(grep -E '^# dma: ' "$log" || true)"
    case "$line" in
        *"cache-clean ok=1"*) ;;
        *)
            echo "# dòng '# dma:' không có 'cache-clean ok=1'"
            bad=1
            ;;
    esac
    lines="$(printf '%s\n' "$line" | sed -n 's/.* lines=\([0-9][0-9]*\).*/\1/p')"
    bytes="$(printf '%s\n' "$line" | sed -n 's/.* bytes=\([0-9][0-9]*\).*/\1/p')"
    line_bytes="$(printf '%s\n' "$line" | sed -n 's/.* line=\([0-9][0-9]*\).*/\1/p')"
    inval_lines="$(printf '%s\n' "$line" | sed -n 's/.* inval_lines=\([0-9][0-9]*\).*/\1/p')"
    if [ -z "$lines" ] || [ "$lines" -le 0 ]; then
        echo "# dòng '# dma:' có lines=${lines:-none} — không có lệnh 'dc civac' nào chạy"
        bad=1
    fi
    if [ -z "$inval_lines" ] || [ "$inval_lines" -le 0 ]; then
        echo "# dòng '# dma:' có inval_lines=${inval_lines:-none} — không có lệnh 'dc ivac' nào chạy"
        bad=1
    fi
    if [ -z "$bytes" ] || [ "$bytes" -le 0 ]; then
        echo "# dòng '# dma:' có bytes=${bytes:-none} — vùng DMA rỗng"
        bad=1
    fi
    if [ -n "$lines" ] && [ -n "$bytes" ] && [ -n "$line_bytes" ] && [ "$line_bytes" -gt 0 ]; then
        if [ $((lines * line_bytes)) -ne "$bytes" ]; then
            echo "# dòng '# dma:' lines×line=$((lines * line_bytes)) khác bytes=$bytes"
            bad=1
        fi
    fi
    return "$bad"
}

# ---------------------------------------------------------------- kiểm dòng `# input:` (KT-283)
# Tám dòng **bằng chứng bàn phím** — đây là phần "virtio-input trên aarch64" của KT-283. Mỗi dòng
# đòi **đúng một** lần (in lặp = bước input chạy hai lần, hoặc log bịa):
#
#   `# input pci: … vendor=0x1af4 device=0x1052 … assigned=0x10600000 readback=0x10600000`
#       BAR gán xong phải **đọc lại** đúng, và đúng leaf **riêng** của input (không đụng blk/net/gpu).
#   `# input name: QEMU Virtio Keyboard`
#       tên **đọc từ** `virtio_input_config` (`select = ID_NAME`) — không phải hằng trong ảnh.
#   `# input dev: … version1=1 rep_feature=0 device_status=0x0f num_queues=2 queue_size=16
#        status_queue_size=16 key_a_bit=1`
#       feature VERSION_1 hai chiều, **không** đàm phán `REP`, **hai** queue `event`/`status`,
#       bitmap `EV_BITS` bật `KEY_A` (thiết bị **có** phím ta sắp bơm).
#   `# input dma: … identity=1 armed=8 status_queue_idle=1`
#       trang DMA map identity (PA **đọc lại từ bảng trang**), 8 đệm arm sẵn, queue `status` bật
#       nhưng **không** arm đệm.
#   `# input: ready` — ảnh in **sau** khi arm; runner bơm phím sau dòng này.
#   `# input: key a code=30 ascii=0x61 shift=0` — `sendkey a` ⇒ `EV_KEY KEY_A 1` ⇒ `'a'`.
#   `# input stats: … errors=0 timeouts=0 id_mismatch=0 clean_lines=<n> inval_lines=<n>`
#       đường đi sạch + cache maintenance **đã chạy** (`n > 0`; bỏ `dc` ⇒ 0 ⇒ ĐỎ).
check_input() {
    local log="$1" bad=0 n line assigned readback lines inval
    for want in \
        '^# input pci: ' \
        '^# input cap: ' \
        '^# input name: ' \
        '^# input dev: ' \
        '^# input dma: ' \
        '^# input: ready$' \
        '^# input: key a ' \
        '^# input stats: '; do
        n="$(grep -cE "$want" "$log" || true)"
        if [ "${n:-0}" -ne 1 ]; then
            echo "# dòng khớp '$want' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        fi
    done
    line="$(grep -E '^# input pci: ' "$log" || true)"
    for want in "vendor=0x1af4" "device=0x1052" "assigned=$INPUT_BAR_ASSIGNED"; do
        case "$line" in
            *"$want"*) ;;
            *)
                echo "# dòng '# input pci:' không có '$want'"
                bad=1
                ;;
        esac
    done
    assigned="$(printf '%s\n' "$line" | sed -n 's/.* assigned=\(0x[0-9a-f]*\).*/\1/p')"
    readback="$(printf '%s\n' "$line" | sed -n 's/.* readback=\(0x[0-9a-f]*\).*/\1/p')"
    if [ -z "$assigned" ] || [ "$assigned" != "$readback" ]; then
        echo "# BAR input đọc lại (${readback:-none}) khác địa chỉ đã gán (${assigned:-none})"
        bad=1
    fi
    line="$(grep -E '^# input name: ' "$log" || true)"
    if [ "$line" != "# input name: QEMU Virtio Keyboard" ]; then
        echo "# tên thiết bị ('${line:-none}') không phải 'QEMU Virtio Keyboard' (đọc từ thiết bị)"
        bad=1
    fi
    line="$(grep -E '^# input dev: ' "$log" || true)"
    for want in "version1=1" "rep_feature=0" "device_status=0x0f" "num_queues=2" \
        "queue_size=16" "status_queue_size=16" "key_a_bit=1"; do
        case "$line" in
            *"$want"*) ;;
            *)
                echo "# dòng '# input dev:' không có '$want'"
                bad=1
                ;;
        esac
    done
    line="$(grep -E '^# input dma: ' "$log" || true)"
    for want in "identity=1" "armed=8" "status_queue_idle=1"; do
        case "$line" in
            *"$want"*) ;;
            *)
                echo "# dòng '# input dma:' không có '$want'"
                bad=1
                ;;
        esac
    done
    line="$(grep -E '^# input: key a ' "$log" || true)"
    for want in "code=30" "ascii=0x61" "shift=0"; do
        case "$line" in
            *"$want"*) ;;
            *)
                echo "# dòng '# input: key a' không có '$want'"
                bad=1
                ;;
        esac
    done
    line="$(grep -E '^# input stats: ' "$log" || true)"
    for want in "errors=0" "timeouts=0" "id_mismatch=0"; do
        case "$line" in
            *"$want"*) ;;
            *)
                echo "# dòng '# input stats:' không có '$want'"
                bad=1
                ;;
        esac
    done
    # Cache maintenance **đã chạy**: `clean_lines`/`inval_lines` đều dương. Bỏ `dc civac`/`dc ivac`
    # ⇒ 0 ⇒ ĐỎ (tầng quan sát được trên QEMU TCG — cùng giới hạn đã ghi ở §11.3).
    lines="$(printf '%s\n' "$line" | sed -n 's/.* clean_lines=\([0-9][0-9]*\).*/\1/p')"
    inval="$(printf '%s\n' "$line" | sed -n 's/.* inval_lines=\([0-9][0-9]*\).*/\1/p')"
    if [ -z "$lines" ] || [ "$lines" -le 0 ]; then
        echo "# dòng '# input stats:' có clean_lines=${lines:-none} — không có lệnh 'dc civac' nào chạy trên vùng DMA"
        bad=1
    fi
    if [ -z "$inval" ] || [ "$inval" -le 0 ]; then
        echo "# dòng '# input stats:' có inval_lines=${inval:-none} — không có lệnh 'dc ivac' nào sau khi thiết bị ghi"
        bad=1
    fi
    return "$bad"
}

# ---------------------------------------------------------------- kiểm TAP của một log
# ---------------------------------------------------------------- kiểm dòng `# core:` (KT-338b)
# Bốn nhóm bằng chứng **máy kiểm** của pha core (ngoài 6 TAP 36..41):
#
#   `# core: addr=0x… size=<n> pin=196608 in_image=1 harts=<1..4> objects=<n> caps=<n> healthy=1`
#       `size` (từ `size_of::<Machine>()`) phải ≤ pin D2-84 **và** `in_image=1` — static trong
#       ảnh, không phải RAM ngoài. `size` đọc từ chính kernel, không phải hằng của runner.
#   `# core configure: priority_old=4 priority_now=9 err0=0 err_ro=3 err_vspace=14 err_entry=0`
#       ca (a): đổi ưu tiên từ 4 → 9; ca (c): quyền trả **3**, `vspace?` trả **14** (`Busy`).
#   `# core ipc: send_err=0 len=8 mr0=0x338b0000 mr3=0x338b0003 mr4=0x338b0004 mr7=0x338b0007
#       server=Ready`
#       ca (b): message 8 MR, MR4..MR7 chỉ có trong **IPC buffer** — mã hoá của chương trình EL0.
#   `# core suspend: switched=1 state=Inactive resume_entry=0x… resume_ok=1 exit=0 deferred=1`
#       ca (d): `Suspend(Defer)` treo ở **điểm an toàn**, `Resume` giao đúng điểm vào, EL0 thoát 0.
#   `# core lock: … irq_before=1 irq_inside=0 irq_after=1 acquired=<n> overlap=0 in_cs=0 ok=1`
#       TAP 41: `DAIF.I` tắt trong vùng găng và **khôi phục** sau khi nhả.
check_core() {
    local log="$1" bad=0 n line size pin
    line="$(grep -E '^# core: addr=0x[0-9a-f]+ size=[0-9]+ pin=[0-9]+ in_image=1 harts=[0-9]+ objects=[0-9]+ caps=[0-9]+ healthy=1$' "$log" || true)"
    if [ -z "$line" ]; then
        echo "# thiếu/bẩn dòng '# core: … healthy=1' (KT-338b)"
        bad=1
    else
        size="$(printf '%s\n' "$line" | sed -n 's/.* size=\([0-9]*\) pin=.*/\1/p')"
        pin="$(printf '%s\n' "$line" | sed -n 's/.* pin=\([0-9]*\) in_image=.*/\1/p')"
        if [ "${size:-0}" -le 0 ] || [ "${size:-0}" -gt "${pin:-0}" ] || [ "${pin:-0}" -ne 196608 ]; then
            echo "# size_of::<Machine>() = ${size:-0} không hợp lệ với pin ${pin:-0} (D2-84)"
            bad=1
        fi
    fi
    n="$(grep -cE '^# core: ' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# core:' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    grep -qE '^# core configure: priority_old=4 priority_now=9 err0=0 err_ro=3 err_vspace=14 err_entry=0 configure=[1-9][0-9]*$' "$log" || {
        echo "# thiếu/bẩn dòng '# core configure:' (ca (a) 4->9 + ca (c) 3/14)"
        bad=1
    }
    grep -qE '^# core ipc: send_err=0 len=8 mr0=0x338b0000 mr3=0x338b0003 mr4=0x338b0004 mr7=0x338b0007 server=Ready$' "$log" || {
        echo "# thiếu/bẩn dòng '# core ipc:' (ca (b): 8 MR, MR4..MR7 từ IPC buffer)"
        bad=1
    }
    grep -qE '^# core suspend: switched=1 state=Inactive resume_entry=0x[0-9a-f]+ resume_ok=1 exit=0 deferred=1 resume_stat=1$' "$log" || {
        echo "# thiếu/bẩn dòng '# core suspend:' (ca (d): Defer + Resume)"
        bad=1
    }
    n="$(grep -cE '^# core switch: nr=0 state=0x0 ' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# core switch:' (điểm an toàn) xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    grep -qE '^# core lock: irq_before=1 irq_inside=0 irq_after=1 acquired=[1-9][0-9]* overlap=0 in_cs=0 ok=1$' "$log" || {
        echo "# thiếu/bẩn dòng '# core lock:' (TAP 41: DAIF.I tắt trong vùng găng, khôi phục sau)"
        bad=1
    }
    return "$bad"
}

# `check_log <log> <marker> <tests>`: 0 ⇔ log đạt TAP + marker + dòng `# smp:` + `# tlbi:` +
# `# umode:` + `# pci:` + `# blk:` + `# net:` + `# core:` + hash ảnh. In lý do đỏ ra stdout.
# `# umode:` + `# pci:` + `# blk:` + `# wx:` + `# dma:` + hash ảnh. In lý do đỏ ra stdout.
check_log() {
    local log="$1" marker="$2" tests="$3" bad=0
    local okn notn mn i

    [ -f "$log" ] || {
        echo "# không có log $log"
        return 1
    }
    grep -qx 'TAP version 13' "$log" || {
        echo "# thiếu dòng 'TAP version 13'"
        bad=1
    }
    grep -qx "1\.\.$tests" "$log" || {
        echo "# thiếu plan '1..$tests'"
        bad=1
    }
    okn="$(grep -c '^ok [0-9][0-9]* - ' "$log" || true)"
    notn="$(grep -c '^not ok ' "$log" || true)"
    if [ "${okn:-0}" -ne "$tests" ]; then
        echo "# có $okn dòng 'ok', cần $tests"
        bad=1
    fi
    if [ "${notn:-0}" -ne 0 ]; then
        echo "# có $notn dòng 'not ok'"
        bad=1
    fi
    i=1
    while [ "$i" -le "$tests" ]; do
        grep -q "^ok $i - " "$log" || {
            echo "# thiếu 'ok $i - ...'"
            bad=1
        }
        i=$((i + 1))
    done
    # Marker **nguyên văn**, đúng **một** lần (0 lần = ảnh không báo boot; 2 lần = in lặp).
    mn="$(grep -Fxc "$marker" "$log" || true)"
    if [ "${mn:-0}" -ne 1 ]; then
        echo "# marker '$marker' xuất hiện ${mn:-0} lần, cần đúng 1"
        bad=1
    fi
    if grep -q '^PANIC' "$log"; then
        echo "# log có dòng PANIC"
        bad=1
    fi
    check_smp "$log" || bad=1
    check_tlbi "$log" || bad=1
    check_umode "$log" || bad=1
    check_pci "$log" || bad=1
    check_blk "$log" || bad=1
    check_net "$log" || bad=1
    check_wx "$log" || bad=1
    check_dma "$log" || bad=1
    check_gpu "$log" || bad=1
    check_input "$log" || bad=1
    check_core "$log" || bad=1
    check_digest "$log" || bad=1
    return "$bad"
}

# ---------------------------------------------------------------- ca âm (bắt buộc)
# Chạy **cùng** `check_log` trên log tốt với kỳ vọng **sai** ⇒ phải ĐỎ. Nếu một ca âm lại XANH thì
# bộ kiểm là "dấu cao su" và runner đang PASS giả.
selftest_negative() {
    local good="$1" start="${2:-1}" fails=0 n="$2" cases=0
    local tmp="$LOGDIR/selftest"
    n=$((start - 1))
    mkdir -p "$tmp" 2>/dev/null || true

    # (a) marker sai
    n=$((n + 1))
    cases=$((cases + 1))
    if check_log "$good" "# viFastOs aarch64: boot BAD" "$EXPECT_TESTS" >"$tmp/a.log" 2>&1; then
        echo "not ok $n - ca âm (a): marker sai mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (a): marker sai ⇒ ĐỎ ($(head -1 "$tmp/a.log"))"
    fi

    # (b) thiếu một dòng `ok` (mô phỏng ảnh in thiếu ca)
    n=$((n + 1))
    cases=$((cases + 1))
    grep -v '^ok 3 - ' "$good" >"$tmp/b.log" 2>/dev/null || true
    if check_log "$tmp/b.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/b.out" 2>&1; then
        echo "not ok $n - ca âm (b): thiếu 'ok 3' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (b): thiếu 'ok 3' ⇒ ĐỎ ($(head -1 "$tmp/b.out"))"
    fi

    # (c) có `not ok`
    n=$((n + 1))
    cases=$((cases + 1))
    sed 's/^ok 2 - /not ok 2 - /' "$good" >"$tmp/c.log" 2>/dev/null || true
    if check_log "$tmp/c.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/c.out" 2>&1; then
        echo "not ok $n - ca âm (c): có 'not ok' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (c): có 'not ok' ⇒ ĐỎ ($(head -1 "$tmp/c.out"))"
    fi

    # (d) plan sai (1..N-1 thay vì 1..N) — dùng $EXPECT_TESTS, không hard-code số ca
    n=$((n + 1))
    cases=$((cases + 1))
    sed "s/^1\.\.$EXPECT_TESTS\$/1..$((EXPECT_TESTS - 1))/" "$good" >"$tmp/d.log" 2>/dev/null || true
    if check_log "$tmp/d.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/d.out" 2>&1; then
        echo "not ok $n - ca âm (d): plan sai mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (d): plan sai ⇒ ĐỎ ($(head -1 "$tmp/d.out"))"
    fi

    # (e) log rỗng (ảnh không in gì — treo trước khi in)
    n=$((n + 1))
    cases=$((cases + 1))
    : >"$tmp/e.log"
    if check_log "$tmp/e.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/e.out" 2>&1; then
        echo "not ok $n - ca âm (e): log rỗng mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (e): log rỗng ⇒ ĐỎ ($(head -1 "$tmp/e.out"))"
    fi

    # (f) thiếu dòng bằng chứng hart 1 online — ảnh chỉ chạy hart 0
    n=$((n + 1))
    cases=$((cases + 1))
    grep -vF '# smp: hart=1 online' "$good" >"$tmp/f.log" 2>/dev/null || true
    if check_log "$tmp/f.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/f.out" 2>&1; then
        echo "not ok $n - ca âm (f): thiếu '# smp: hart=1 online' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (f): thiếu dòng hart 1 online ⇒ ĐỎ ($(head -1 "$tmp/f.out"))"
    fi

    # (g) dòng IPI xuất hiện **hai** lần — hart 1 nhận hai IPI (hoặc in lặp)
    n=$((n + 1))
    cases=$((cases + 1))
    awk '{print} /^# smp: ipi recv=1/{print}' "$good" >"$tmp/g.log" 2>/dev/null || true
    if check_log "$tmp/g.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/g.out" 2>&1; then
        echo "not ok $n - ca âm (g): '# smp: ipi recv=1' hai lần mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (g): IPI in hai lần ⇒ ĐỎ ($(head -1 "$tmp/g.out"))"
    fi

    # (h) **KT-262** — thiếu **cả** dòng `# tlbi:` (shootdown không chạy / không ack)
    n=$((n + 1))
    cases=$((cases + 1))
    grep -vF '# tlbi: ' "$good" >"$tmp/h.log" 2>/dev/null || true
    if check_log "$tmp/h.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/h.out" 2>&1; then
        echo "not ok $n - ca âm (h): thiếu dòng '# tlbi:' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (h): thiếu dòng '# tlbi:' ⇒ ĐỎ ($(head -1 "$tmp/h.out"))"
    fi

    # (i) **KT-262** — shootdown **thiếu ack** (`acks=N-1` thay vì `acks=N`)
    n=$((n + 1))
    cases=$((cases + 1))
    sed "s/acks=$EXPECT_ACKS /acks=$((EXPECT_ACKS - 1)) /" \
        "$good" >"$tmp/i.log" 2>/dev/null || true
    if check_log "$tmp/i.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/i.out" 2>&1; then
        echo "not ok $n - ca âm (i): thiếu ack shootdown mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (i): thiếu ack shootdown ⇒ ĐỎ ($(head -1 "$tmp/i.out"))"
    fi

    # (j) **KT-265** — hash ảnh trong log **lệch** ELF (ảnh khác/không tự khai đúng)
    n=$((n + 1))
    cases=$((cases + 1))
    sed 's/^# conf image: build=[0-9a-f]*/# conf image: build=ffffffffffffffff/' \
        "$good" >"$tmp/j.log" 2>/dev/null || true
    if check_log "$tmp/j.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/j.out" 2>&1; then
        echo "not ok $n - ca âm (j): hash ảnh lệch ELF mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (j): hash ảnh lệch ELF ⇒ ĐỎ ($(head -1 "$tmp/j.out"))"
    fi

    # (k) **KT-269** — thiếu **cả** dòng `# umode:` (ảnh không `eret` xuống EL0 / không xử lý `svc`)
    n=$((n + 1))
    cases=$((cases + 1))
    grep -vF '# umode: ' "$good" >"$tmp/k.log" 2>/dev/null || true
    if check_log "$tmp/k.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/k.out" 2>&1; then
        echo "not ok $n - ca âm (k): thiếu dòng '# umode:' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (k): thiếu dòng '# umode:' ⇒ ĐỎ ($(head -1 "$tmp/k.out"))"
    fi

    # (l) **KT-273** — **không có đĩa**: ảnh phải in `# blk: no device` và TAP phải ĐỎ.
    # Đây là ca âm **thật** (boot QEMU lần hai, không `-drive`), không phải biến đổi log: nó
    # chứng minh driver **fail-closed** khi thiếu thiết bị, thay vì đọc ra số 0 rồi in `ok`.
    n=$((n + 1))
    cases=$((cases + 1))
    out="$LOGDIR/boot-nodisk.log"
    if ! qg_boot "$out" "$TIMEOUT_DS" "$QEMU" -M virt,gic-version=3 -cpu cortex-a72 -m 128M \
        -nographic -smp "$EXPECT_SMP" -kernel "$IMAGE" -no-reboot -nic none \
        -device virtio-gpu-pci; then
        echo "not ok $n - ca âm (l): boot không có đĩa bị treo/timeout"
        fails=$((fails + 1))
    elif check_log "$out" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/l.out" 2>&1; then
        echo "not ok $n - ca âm (l): không có đĩa mà kiểm vẫn XANH"
        fails=$((fails + 1))
    elif ! grep -Fq '# blk: no device' "$out"; then
        echo "not ok $n - ca âm (l): thiếu dòng '# blk: no device'"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (l): không có đĩa ⇒ '# blk: no device' + TAP ĐỎ ($(head -1 "$tmp/l.out"))"
    fi

    # (m) **KT-275** — **không có mạng**: ảnh phải in `# net: no device` và TAP phải ĐỎ.
    # Ca âm **thật** (boot QEMU lần ba, có đĩa nhưng **không** `-netdev`/`-device virtio-net-pci`):
    # chứng minh driver mạng **fail-closed** khi thiếu thiết bị, thay vì in `ok` cho một card
    # không tồn tại. Đĩa vẫn có để `blk` xanh — nếu không, ảnh dừng ở `blk` và **không** in dòng
    # `# net:` nào, tức ca âm sẽ đỏ vì lý do sai.
    n=$((n + 1))
    cases=$((cases + 1))
    out="$LOGDIR/boot-nonet.log"
    if ! qg_boot "$out" "$TIMEOUT_DS" "$QEMU" -M virt,gic-version=3 -cpu cortex-a72 -m 128M \
        -nographic -smp "$EXPECT_SMP" -kernel "$IMAGE" -no-reboot -nic none \
        -drive "file=$DISK,if=none,id=d0,format=raw,snapshot=on" \
        -device virtio-blk-pci,drive=d0 -device virtio-gpu-pci; then
        echo "not ok $n - ca âm (m): boot không có mạng bị treo/timeout"
        fails=$((fails + 1))
    elif check_log "$out" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/m.out" 2>&1; then
        echo "not ok $n - ca âm (m): không có mạng mà kiểm vẫn XANH"
        fails=$((fails + 1))
    elif ! grep -Fq '# net: no device' "$out"; then
        echo "not ok $n - ca âm (m): thiếu dòng '# net: no device'"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (m): không có mạng ⇒ '# net: no device' + TAP ĐỎ ($(head -1 "$tmp/m.out"))"
    fi

    # (n) **KT-277** — thiếu **cả** dòng `# wx:` (ảnh không tách `.text` RX / `.data` RW, hoặc
    # không thử ghi vào `.text`). TAP 22 vẫn có thể `ok` nếu ảnh tự khai — dòng bằng chứng mới là
    # thứ bộ kiểm đòi, nên thiếu nó phải ĐỎ.
    n=$((n + 1))
    cases=$((cases + 1))
    grep -vF '# wx: ' "$good" >"$tmp/m.log" 2>/dev/null || true
    if check_log "$tmp/m.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/m.out" 2>&1; then
        echo "not ok $n - ca âm (n): thiếu dòng '# wx:' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (n): thiếu dòng '# wx:' ⇒ ĐỎ ($(head -1 "$tmp/m.out"))"
    fi

    # (o) **KT-277** — thiếu **cả** dòng `# dma:` (ảnh không chạy cache maintenance trước DMA).
    n=$((n + 1))
    cases=$((cases + 1))
    grep -vF '# dma: ' "$good" >"$tmp/n.log" 2>/dev/null || true
    if check_log "$tmp/n.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/n.out" 2>&1; then
        echo "not ok $n - ca âm (o): thiếu dòng '# dma:' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (o): thiếu dòng '# dma:' ⇒ ĐỎ ($(head -1 "$tmp/n.out"))"
    fi

    # (p) **KT-279** — **không có GPU**: ảnh phải in `# gpu: no device` và TAP phải ĐỎ.
    # Ca âm **thật** (boot QEMU lần tư, có đĩa + mạng nhưng **không** `-device virtio-gpu-pci`):
    # chứng minh driver GPU **fail-closed** khi thiếu thiết bị, thay vì in `ok` cho một card không
    # tồn tại. Đĩa + mạng vẫn có để `blk`/`net` xanh — nếu không, ảnh dừng ở bước trước và **không**
    # in dòng `# gpu:` nào, tức ca âm sẽ đỏ vì lý do sai.
    n=$((n + 1))
    cases=$((cases + 1))
    out="$LOGDIR/boot-nogpu.log"
    srv_log="$LOGDIR/kt289-tcp-server-nogpu.log"
    # **KT-289**: boot này **có** mạng nên khách chạy tới TCP trước khi hỏng ở GPU — server phải
    # có, nếu không TAP 33/34 đỏ vì lý do sai và `# gpu: no device` không bao giờ được in.
    if ! start_tcp_server "$srv_log"; then
        echo "not ok $n - ca âm (p): không dựng được TCP server trên 127.0.0.1:$TCP_PORT"
        fails=$((fails + 1))
        stop_tcp_server
    elif ! qg_boot "$out" "$TIMEOUT_DS" "$QEMU" -M virt,gic-version=3 -cpu cortex-a72 -m 128M \
        -nographic -smp "$EXPECT_SMP" -kernel "$IMAGE" -no-reboot -nic none \
        -netdev user,id=n0 -device virtio-net-pci,netdev=n0 \
        -drive "file=$DISK,if=none,id=d0,format=raw,snapshot=on" \
        -device virtio-blk-pci,drive=d0; then
        echo "not ok $n - ca âm (p): boot không có GPU bị treo/timeout"
        fails=$((fails + 1))
        stop_tcp_server
    elif ! wait_tcp_server "$srv_log"; then
        echo "not ok $n - ca âm (p): server TCP không có đủ bằng chứng hai chiều (log $srv_log)"
        fails=$((fails + 1))
        stop_tcp_server
    elif check_log "$out" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/p.out" 2>&1; then
        echo "not ok $n - ca âm (p): không có GPU mà kiểm vẫn XANH"
        fails=$((fails + 1))
        stop_tcp_server
    elif ! grep -Fq '# gpu: no device' "$out"; then
        echo "not ok $n - ca âm (p): thiếu dòng '# gpu: no device'"
        fails=$((fails + 1))
        stop_tcp_server
    else
        stop_tcp_server
        echo "ok $n - ca âm (p): không có GPU ⇒ '# gpu: no device' + TAP ĐỎ ($(head -1 "$tmp/p.out"))"
    fi

    # (q) **KT-283** — **không có bàn phím**: ảnh phải in `# input: no device` và TAP phải ĐỎ.
    # Ca âm **thật** (boot QEMU lần năm, có đĩa + mạng + GPU nhưng **không**
    # `-device virtio-keyboard-pci`): chứng minh driver bàn phím **fail-closed** khi thiếu thiết bị,
    # thay vì in `ok` cho một bàn phím không tồn tại. Ba thiết bị kia vẫn có để `blk`/`net`/`gpu`
    # xanh — nếu không, ảnh dừng ở bước trước và **không** in dòng `# input:` nào, tức ca âm sẽ đỏ
    # vì lý do sai.
    n=$((n + 1))
    cases=$((cases + 1))
    out="$LOGDIR/boot-nokbd.log"
    srv_log="$LOGDIR/kt289-tcp-server-nokbd.log"
    # **KT-289**: như (p) — boot này có mạng nên khách chạy tới TCP trước khi hỏng ở bàn phím.
    if ! start_tcp_server "$srv_log"; then
        echo "not ok $n - ca âm (q): không dựng được TCP server trên 127.0.0.1:$TCP_PORT"
        fails=$((fails + 1))
        stop_tcp_server
    elif ! qg_boot "$out" "$TIMEOUT_DS" "$QEMU" -M virt,gic-version=3 -cpu cortex-a72 -m 128M \
        -nographic -smp "$EXPECT_SMP" -kernel "$IMAGE" -no-reboot -nic none \
        -netdev user,id=n0 -device virtio-net-pci,netdev=n0 \
        -drive "file=$DISK,if=none,id=d0,format=raw,snapshot=on" \
        -device virtio-blk-pci,drive=d0 -device virtio-gpu-pci; then
        echo "not ok $n - ca âm (q): boot không có bàn phím bị treo/timeout"
        fails=$((fails + 1))
        stop_tcp_server
    elif ! wait_tcp_server "$srv_log"; then
        echo "not ok $n - ca âm (q): server TCP không có đủ bằng chứng hai chiều (log $srv_log)"
        fails=$((fails + 1))
        stop_tcp_server
    elif check_log "$out" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/q.out" 2>&1; then
        echo "not ok $n - ca âm (q): không có bàn phím mà kiểm vẫn XANH"
        fails=$((fails + 1))
        stop_tcp_server
    elif ! grep -Fq '# input: no device' "$out"; then
        echo "not ok $n - ca âm (q): thiếu dòng '# input: no device'"
        fails=$((fails + 1))
        stop_tcp_server
    else
        stop_tcp_server
        echo "ok $n - ca âm (q): không có bàn phím ⇒ '# input: no device' + TAP ĐỎ ($(head -1 "$tmp/q.out"))"
    fi

    # (r) **KT-289** — thiếu **cả** dòng `# net: tcp` (ảnh không mở TCP / không nhận dữ liệu).
    # TAP 33/34 vẫn có thể `ok` nếu ảnh tự khai — dòng bằng chứng mới là thứ bộ kiểm đòi.
    n=$((n + 1))
    cases=$((cases + 1))
    grep -vF '# net: tcp ' "$good" | grep -vF '# net tcp data: ' >"$tmp/r.log" 2>/dev/null || true
    if check_log "$tmp/r.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/r.out" 2>&1; then
        echo "not ok $n - ca âm (r): thiếu dòng '# net: tcp' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (r): thiếu dòng '# net: tcp' ⇒ ĐỎ ($(head -1 "$tmp/r.out"))"
    fi

    # (s) **KT-289** — thiếu **cả** dòng `# net: dns` (ảnh không truy vấn DNS / không có bản ghi A).
    n=$((n + 1))
    cases=$((cases + 1))
    grep -vF '# net: dns ' "$good" >"$tmp/s.log" 2>/dev/null || true
    if check_log "$tmp/s.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/s.out" 2>&1; then
        echo "not ok $n - ca âm (s): thiếu dòng '# net: dns' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (s): thiếu dòng '# net: dns' ⇒ ĐỎ ($(head -1 "$tmp/s.out"))"
    fi

    # (t) **KT-294** — **cả hai** lần thử timeout: ảnh in `# net dns: … attempts=2 err=Timeout` +
    # `# net: no dns` (TAP 35 đỏ). Log **giả** ở đây vẫn giữ `ok 35` — đúng kiểu "PASS giả" mà bộ
    # kiểm phải bắt: có `# net: no dns` là ĐỎ, dù TAP tự khai xanh.
    n=$((n + 1))
    cases=$((cases + 1))
    sed -e 's/^# net dns: name=example\.com .*$/# net dns: elapsed_ms=6000 attempts=2 err=Timeout/' \
        -e 's/^# net: dns example\.com .*$/# net: no dns/' "$good" >"$tmp/t.log" 2>/dev/null || true
    if check_log "$tmp/t.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/t.out" 2>&1; then
        echo "not ok $n - ca âm (t): cả hai lần thử timeout ('# net: no dns') mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (t): cả hai lần thử timeout ⇒ ĐỎ ($(head -1 "$tmp/t.out"))"
    fi

    # (u) **KT-294** — **retransmit vô hạn**: `attempts=3` phải bị từ chối (trần là 2). Đột biến
    # "bỏ trần" ở driver sẽ in `attempts=3` (hoặc treo tới timeout boot) — cả hai đều ĐỎ.
    n=$((n + 1))
    cases=$((cases + 1))
    # **KT-303** — đột biến phải khớp **cả** `attempts=1` **và** `attempts=2`: lần chạy thật có
    # `attempts=2` khi lần thử đầu timeout (đo được ở `/tmp/a64-r17d.log`), nên mẫu cũ chỉ khớp
    # `attempts=1` ⇒ `sed` không đổi gì ⇒ ca âm (u) **XANH giả**. Dùng `attempts=[12]`.
    sed -E 's/ attempts=[12] const:timeout_ms=3000$/ attempts=3 const:timeout_ms=3000/' "$good" \
        >"$tmp/u.log" 2>/dev/null || true
    if check_log "$tmp/u.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/u.out" 2>&1; then
        echo "not ok $n - ca âm (u): attempts=3 (retransmit vô hạn) mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (u): attempts=3 ⇒ ĐỎ ($(head -1 "$tmp/u.out"))"
    fi

    # (v) **KT-302** — quét **thiếu bus** (`buses=16`, hành vi cũ của KT-273): runner phải ĐỎ.
    # Đây là đột biến "map thiếu bus" ở tầng log: nếu `check_pci` không đòi `buses=256`, một ảnh
    # chỉ quét 16 bus vẫn PASS — đúng lỗi KT-302 sinh ra để đóng.
    n=$((n + 1))
    cases=$((cases + 1))
    sed 's/ buses=256$/ buses=16/' "$good" >"$tmp/v.log" 2>/dev/null || true
    if check_log "$tmp/v.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/v.out" 2>&1; then
        echo "not ok $n - ca âm (v): buses=16 (quét thiếu bus) mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (v): buses=16 ⇒ ĐỎ ($(head -1 "$tmp/v.out"))"
    fi

    # (w) **KT-302** — ECAM map bằng **lá 4 KiB** thay block (`ecam_block_ok=0`): runner phải ĐỎ.
    # Nếu không đòi `ecam_block_ok=1`, một ảnh map 256 MiB bằng 128 bảng mức 3 (nếu arena đủ) vẫn
    # PASS — trong khi yêu cầu là **block descriptor**.
    n=$((n + 1))
    cases=$((cases + 1))
    sed 's/ ecam_block_ok=1 / ecam_block_ok=0 /' "$good" >"$tmp/w.log" 2>/dev/null || true
    if check_log "$tmp/w.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/w.out" 2>&1; then
        echo "not ok $n - ca âm (w): ecam_block_ok=0 (map bằng lá 4 KiB) mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (w): ecam_block_ok=0 ⇒ ĐỎ ($(head -1 "$tmp/w.out"))"
    fi

    # (x) **KT-305** — `unmap` block **không chia nhỏ** (`blocks_split=0`, tức xoá cả block):
    # runner phải ĐỎ. Nếu không đòi `blocks_split=1`, một ảnh "unmap block = xoá block" (mất
    # mapping 511 trang kế cận) vẫn PASS — đúng lỗi KT-305 sinh ra để đóng.
    n=$((n + 1))
    cases=$((cases + 1))
    sed 's/ blocks_split=1$/ blocks_split=0/' "$good" >"$tmp/x.log" 2>/dev/null || true
    if check_log "$tmp/x.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/x.out" 2>&1; then
        echo "not ok $n - ca âm (x): blocks_split=0 (không chia nhỏ block) mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (x): blocks_split=0 ⇒ ĐỎ ($(head -1 "$tmp/x.out"))"
    fi

    # (y) **KT-338c** — thiếu **cả** dòng `# ipi: mailbox` (hộp thư IPI bitmask dùng chung không được nối
    # vào SGI, hoặc ảnh không in bằng chứng). TAP 11 vẫn `ok` nếu chỉ đếm IPI ⇒ phải ĐỎ ở đây.
    n=$((n + 1))
    cases=$((cases + 1))
    grep -vF '# ipi: mailbox ' "$good" >"$tmp/y.log" 2>/dev/null || true
    if check_log "$tmp/y.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/y.out" 2>&1; then
        echo "not ok $n - ca âm (y): thiếu dòng '# ipi: mailbox' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (y): thiếu dòng '# ipi: mailbox' ⇒ ĐỎ ($(head -1 "$tmp/y.out"))"
    fi

    # (z) **KT-338b** — thiếu dòng `# core ipc:` của pha core (IPC từ EL0 không chạy, hoặc ảnh
    # không in bằng chứng): 6 TAP 36..41 có thể vẫn `ok` nếu chỉ tin chương trình EL0 ⇒ phải ĐỎ.
    n=$((n + 1))
    cases=$((cases + 1))
    grep -vF '# core ipc: send_err=' "$good" >"$tmp/z.log" 2>/dev/null || true
    if check_log "$tmp/z.log" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$tmp/z.out" 2>&1; then
        echo "not ok $n - ca âm (z): thiếu dòng '# core ipc:' mà kiểm vẫn XANH"
        fails=$((fails + 1))
    else
        echo "ok $n - ca âm (z): thiếu dòng '# core ipc:' ⇒ ĐỎ ($(head -1 "$tmp/z.out"))"
    fi

    NEG_CASES=$cases
    echo "# ca âm: $((cases - fails))/$cases ĐỎ đúng như phải"
    [ "$fails" -eq 0 ]
}

# ---------------------------------------------------------------- khởi động
command -v cargo >/dev/null 2>&1 || die "không có cargo"
QEMU="$(command -v qemu-system-aarch64 || true)"
[ -n "$QEMU" ] || die "không có qemu-system-aarch64"
OBJDUMP="$(find_objdump)" || die "không có objdump (llvm-objdump/rust-objdump/xcrun/objdump)"
# **KT-279**: `python3` để gửi `screendump` qua monitor + chạy `gpu_ppm.py` (không cần 3.12).
PYTHON="$(command -v python3 || true)"
[ -n "$PYTHON" ] || die "không có python3 (cần cho monitor screendump + gpu_ppm.py)"
[ -f "$PPM_CHECK" ] || die "không thấy $PPM_CHECK"

QEMU_VERSION="$("$QEMU" --version 2>/dev/null | sed -n '1s/.*version \([0-9][^ ]*\).*/\1/p')"
[ -n "$QEMU_VERSION" ] || die "không đọc được phiên bản QEMU"
echo "# qemu $QEMU_VERSION  objdump $OBJDUMP"
echo "# target $TARGET  image $IMAGE"

# `qemu_guard.sh` (KT-78): process group riêng + timeout kill cả group + trap dọn con.
# **Không** dùng `perl -e 'alarm N; exec @ARGV'`: QEMU bỏ qua SIGALRM (đo 2026-11-21).
. "$HERE/qemu_guard.sh"
qg_init || die "qemu_guard: không khởi tạo được (thiếu perl?)"
QG_LOG="$LOGDIR/qemu-guard.log"
rm -f "$QG_LOG" 2>/dev/null

echo "# build ảnh SẢN PHẨM aarch64 ($TARGET, release, KHÔNG feature)"
if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-aarch64 --target "$TARGET" --release \
    >"$LOGDIR/build.log" 2>&1; then
    tail -20 "$LOGDIR/build.log" >&2
    die "build ảnh sản phẩm aarch64 FAIL (xem $LOGDIR/build.log)"
fi
[ -f "$IMAGE" ] || die "không thấy ảnh sản phẩm $IMAGE"
cp "$IMAGE" "$PRODUCT_IMAGE" || die "không copy được ảnh sản phẩm"
# Ngân sách `.data+.bss` của **ảnh sản phẩm** (D2-236/D2-244): trần **64 KiB**, không nới.
if ! bash "$HERE/budget-aarch64.sh" "$PRODUCT_IMAGE" "aarch64-product" --product \
    >"$LOGDIR/budget-product.log" 2>&1; then
    cat "$LOGDIR/budget-product.log"
    die "ngân sách ảnh SẢN PHẨM vượt trần 64 KiB (xem $LOGDIR/budget-product.log)"
fi
echo "# product $(grep -E '^# image=' "$LOGDIR/budget-product.log")"

echo "# build ảnh KTEST aarch64 ($TARGET, release, --features ktest)"
if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-aarch64 --target "$TARGET" --release \
    --features ktest >"$LOGDIR/build-ktest.log" 2>&1; then
    tail -20 "$LOGDIR/build-ktest.log" >&2
    die "build ảnh ktest aarch64 FAIL (xem $LOGDIR/build-ktest.log)"
fi
[ -f "$IMAGE" ] || die "không thấy ảnh ktest $IMAGE"
cp "$IMAGE" "$KTEST_IMAGE" || die "không copy được ảnh ktest"
if ! bash "$HERE/budget-aarch64.sh" "$KTEST_IMAGE" "aarch64-ktest" \
    >"$LOGDIR/budget-ktest.log" 2>&1; then
    cat "$LOGDIR/budget-ktest.log"
    die "ngân sách ảnh KTEST vượt trần (xem $LOGDIR/budget-ktest.log)"
fi
echo "# ktest $(grep -E '^# image=' "$LOGDIR/budget-ktest.log")"
# Từ đây mọi phép kiểm TAP dùng **ảnh ktest** (ảnh sản phẩm được kiểm riêng ở mục KT-338c).
IMAGE="$KTEST_IMAGE"

# **KT-265 / D2-80** — hash ảnh mà kernel **tự in** phải khớp bản băm lại từ tệp ELF. Băm **cùng**
# section `.text` bằng `image_digest.py` (cùng hằng FNV-1a 64 với `vifast_plat_aarch64::image`).
command -v python3.12 >/dev/null 2>&1 || die "không có python3.12 (cần cho image_digest.py)"
[ -f "$DIGEST" ] || die "không thấy $DIGEST"
DIGEST_OUT="$(python3.12 "$DIGEST" "$IMAGE" 2>/dev/null)" || die "image_digest.py lỗi trên $IMAGE"
WANT_BUILD="$(printf '%s\n' "$DIGEST_OUT" | sed -n 's/^build=\([0-9a-f]*\).*/\1/p')"
WANT_TEXT="$(printf '%s\n' "$DIGEST_OUT" | sed -n 's/.* text=\([0-9]*\)$/\1/p')"
[ -n "$WANT_BUILD" ] && [ -n "$WANT_TEXT" ] || die "không đọc được BUILD_ID từ $DIGEST_OUT"
echo "# BUILD_ID (D2-80) = build=$WANT_BUILD text=$WANT_TEXT"

fails=0
total=0

# =============================================================================================
# **KT-338c** (D2-08) — ảnh SẢN PHẨM: 0 harness marker, boot tới marker + halt, neo ảnh D2-80.
# Bốn ca: (P1) quét `strings` == 0 **kèm ca âm** (ảnh ktest phải > 0), (P2) boot thật, (P3) log
# sản phẩm **phải bị** bộ kiểm TAP từ chối, (P4) log ktest **phải bị** bộ kiểm sản phẩm từ chối.
# Không có (P4) thì "bộ kiểm sản phẩm" chỉ là `grep marker` — không phải phép kiểm tách ảnh.
# =============================================================================================

# `harness_hits <ảnh>`: tổng số chuỗi harness (grep -F) trong ảnh.
harness_hits() {
    local img="$1" pat n total_hits=0
    if command -v strings >/dev/null 2>&1; then
        for pat in "${PRODUCT_HARNESS_MARKERS[@]}"; do
            n="$(strings -a "$img" 2>/dev/null | grep -Fc -- "$pat" || true)"
            total_hits=$((total_hits + ${n:-0}))
        done
    else
        for pat in "${PRODUCT_HARNESS_MARKERS[@]}"; do
            n="$(LC_ALL=C grep -a -o -F -- "$pat" "$img" 2>/dev/null | wc -l | tr -d ' ' || true)"
            total_hits=$((total_hits + ${n:-0}))
        done
    fi
    echo "$total_hits"
}

# `check_product_log <log> <marker>`: log **sản phẩm** — marker đúng 1 lần + halt + `# conf image:`
# + **không** dòng TAP nào. Ảnh ktest (có TAP) **phải** đỏ ở đây — đó là ca âm (P4).
check_product_log() {
    local log="$1" marker="$2" bad=0 n
    [ -f "$log" ] || {
        echo "# không có log $log"
        return 1
    }
    n="$(grep -cxF -- "$marker" "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# marker sản phẩm xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    grep -qxF -- "$HALT_LINE" "$log" || {
        echo "# thiếu dòng halt '$HALT_LINE'"
        bad=1
    }
    grep -qE '^# conf image: build=[0-9a-f]{16} text=[0-9]+$' "$log" || {
        echo "# thiếu '# conf image: build=<16 hex> text=<len>' (D2-80)"
        bad=1
    }
    if grep -qE '^(TAP version 13|not ok|ok [0-9]+ - |1\.\.[0-9]+)' "$log"; then
        echo "# log SẢN PHẨM chứa dấu vết harness (TAP)"
        bad=1
    fi
    return $bad
}

# (P1) quét chuỗi harness: sản phẩm 0, **ktest > 0** (ca âm — bộ quét có răng).
total=$((total + 1))
PROD_HITS="$(harness_hits "$PRODUCT_IMAGE")"
KTEST_HITS="$(harness_hits "$KTEST_IMAGE")"
if [ "${PROD_HITS:-1}" -eq 0 ] && [ "${KTEST_HITS:-0}" -gt 0 ]; then
    echo "ok $total - D2-08: ảnh sản phẩm 0 harness marker (ktest=$KTEST_HITS ⇒ bộ quét có răng)"
else
    echo "not ok $total - D2-08: sản phẩm=$PROD_HITS (cần 0), ktest=$KTEST_HITS (cần >0)"
    fails=$((fails + 1))
fi

# (P2) boot ảnh sản phẩm: marker 1 lần + halt + không TAP + `# conf image` khớp ELF (D2-80).
total=$((total + 1))
PROD_LOG="$LOGDIR/product-boot.log"
if ! qg_boot "$PROD_LOG" "$TIMEOUT_DS" "$QEMU" -M virt,gic-version=3 -cpu cortex-a72 -m 128M \
    -nographic -smp 1 -net none -kernel "$PRODUCT_IMAGE"; then
    echo "not ok $total - sản phẩm: boot quá $((TIMEOUT_DS / 10)) s (pid=$QG_LAST_PID), log $PROD_LOG"
    fails=$((fails + 1))
else
    # `image_digest.py` in `build=<16 hex> text=<len>` — cùng bộ băm D2-80 với ảnh ktest.
    DIGEST_OUT_PROD="$(python3.12 "$DIGEST" "$PRODUCT_IMAGE" 2>/dev/null || true)"
    WANT_PROD_CONF="# conf image: ${DIGEST_OUT_PROD}"
    prod_ok=0
    check_product_log "$PROD_LOG" "$EXPECT_MARKER" >"$LOGDIR/product-check.log" 2>&1 || prod_ok=1
    if [ -n "$WANT_PROD_CONF" ] && ! grep -qxF -- "$WANT_PROD_CONF" "$PROD_LOG"; then
        echo "# '# conf image:' không khớp ELF (cần '$WANT_PROD_CONF')" >>"$LOGDIR/product-check.log"
        prod_ok=1
    fi
    if [ "$prod_ok" -eq 0 ]; then
        echo "ok $total - sản phẩm: marker 1 lần + halt + 0 TAP + '# conf image' khớp ELF (D2-80)"
    else
        echo "not ok $total - sản phẩm: $(head -2 "$LOGDIR/product-check.log" | tr '\n' ' ')"
        fails=$((fails + 1))
    fi
fi

# (P3) ca âm của **bộ kiểm sản phẩm**: đưa log sản phẩm cho `check_log` (TAP) ⇒ **phải ĐỎ**.
total=$((total + 1))
if check_log "$PROD_LOG" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$LOGDIR/product-neg-tap.log" 2>&1; then
    echo "not ok $total - ca âm: log SẢN PHẨM mà bộ kiểm TAP vẫn XANH"
    fails=$((fails + 1))
else
    echo "ok $total - ca âm: log sản phẩm bị bộ kiểm TAP từ chối (không TAP nào)"
fi

# (P4) ca âm của **bộ kiểm sản phẩm**: log **ktest** (có TAP 35) ⇒ `check_product_log` phải ĐỎ.
# Chạy sau vòng boot đầu; `$LOGDIR/boot-1.log` là log ktest. Nếu hàm này xanh trên log ktest thì
# nó chỉ là "grep marker", không phải phép kiểm tách ảnh.
product_neg_p4() {
    local log="$LOGDIR/boot-1.log"
    [ -f "$log" ] || return 1
    total=$((total + 1))
    if check_product_log "$log" "$EXPECT_MARKER" >"$LOGDIR/product-neg-ktest.log" 2>&1; then
        echo "not ok $total - ca âm: log ktest bị bộ kiểm sản phẩm NHẬN (có TAP)"
        fails=$((fails + 1))
    else
        echo "ok $total - ca âm: log ktest bị bộ kiểm sản phẩm từ chối (có TAP)"
    fi
}

# ---- kiểm 1: ELF (T-objdump)
total=$((total + 1))
if check_elf "$IMAGE" >"$LOGDIR/elf.log" 2>&1; then
    echo "ok $total - elf: .text tại $EXPECT_TEXT_VMA, .text.vectors TEXT (không BSS), 16 stub đúng offset"
else
    echo "not ok $total - elf: $(head -3 "$LOGDIR/elf.log" | tr '\n' ' ')"
    fails=$((fails + 1))
fi

# ---- kiểm 2..: boot N lần
#
# **KT-279 + KT-283**: **mọi** boot đi qua `boot_full` (mở monitor + `-device virtio-gpu-pci` +
# `-device virtio-keyboard-pci`, chờ marker của khách rồi `screendump`, chờ `# input: ready` rồi
# `sendkey a`). Trước KT-283 boot 2..N dùng `qg_boot` (không monitor) — nhưng ảnh giờ **cần** phím
# ở mọi boot, nên một đường duy nhất để "ảnh nào" không lệch giữa boot 1 và boot N.
i=1
while [ "$i" -le "$N_BOOT" ]; do
    total=$((total + 1))
    out="$LOGDIR/boot-$i.log"
    ppm="$LOGDIR/boot-$i.ppm"
    srv_log="$LOGDIR/kt289-tcp-server-$i.log"
    # **KT-289**: server TCP **thật** phải lắng nghe **trước** khi khách bắt tay. Không dựng được
    # ⇒ boot đó ĐỎ (không có đường "TCP xanh nhờ timeout").
    if ! start_tcp_server "$srv_log"; then
        echo "not ok $total - boot $i: không dựng được TCP server trên 127.0.0.1:$TCP_PORT"
        fails=$((fails + 1))
        stop_tcp_server
        i=$((i + 1))
        continue
    fi
    boot_full "$out" "$ppm"
    rc=$?
    if [ "$rc" -ne 0 ]; then
        echo "not ok $total - boot $i: quá $((TIMEOUT_DS / 10)) s (treo, pid=$QG_LAST_PID), log $out"
        fails=$((fails + 1))
        stop_tcp_server
        i=$((i + 1))
        continue
    fi
    # Bằng chứng **hai chiều**: log server phải có `accept` + `sent=13` + `recv=15`.
    if ! wait_tcp_server "$srv_log"; then
        echo "not ok $total - boot $i: server TCP không có đủ bằng chứng hai chiều (log $srv_log)"
        fails=$((fails + 1))
        stop_tcp_server
        i=$((i + 1))
        continue
    fi
    stop_tcp_server
    if check_log "$out" "$EXPECT_MARKER" "$EXPECT_TESTS" >"$LOGDIR/tap-$i.log" 2>&1; then
        echo "ok $total - boot $i: TAP $EXPECT_TESTS/$EXPECT_TESTS + marker 1 lần + hash ảnh khớp ELF"
    else
        echo "not ok $total - boot $i: $(head -3 "$LOGDIR/tap-$i.log" | tr '\n' ' '), log $out"
        fails=$((fails + 1))
    fi
    # **KT-279**: ảnh chụp màn hình của boot 1 — bằng chứng **thật** rằng GPU đã vẽ (điểm ảnh +
    # từng ký tự console), độc lập với TAP và với framebuffer trong RAM.
    if [ "$i" -eq 1 ]; then
        total=$((total + 1))
        if check_ppm "$ppm" >"$LOGDIR/ppm-$i.log" 2>&1; then
            echo "ok $total - ppm: $(grep -c '^ok ' "$LOGDIR/ppm-$i.log" || true) kiểm PPM xanh (màu rect + 29/29 ký tự console)"
        else
            echo "not ok $total - ppm: $(head -3 "$LOGDIR/ppm-$i.log" | tr '\n' ' '), ảnh $ppm"
            fails=$((fails + 1))
        fi
    fi
    i=$((i + 1))
done

# (P4) — ca âm của bộ kiểm sản phẩm trên log **ktest** (KT-338c).
product_neg_p4

# ---- ca âm (bắt buộc, chạy **mặc định**): bộ kiểm phải ĐỎ khi kỳ vọng sai
good="$LOGDIR/boot-1.log"
if [ ! -f "$good" ]; then
    echo "not ok $((total + 1)) - ca âm: không có log tốt để chạy (boot 1 không chạy được)"
    fails=$((fails + 1))
    total=$((total + 1))
else
    # Ca âm đánh số **tiếp** dãy của runner để log TAP không trùng số hiệu.
    # Số ca đếm **từ chính hàm** (`NEG_CASES`) — trước KT-279 chỗ này cộng cứng 13+14 = 27 trong
    # khi hàm chỉ chạy 15 ca, nên dòng tổng kết phồng lên. Không đổi ca nào, chỉ đếm đúng.
    NEG_CASES=0
    selftest_negative "$good" "$((total + 1))" || fails=$((fails + 1))
    total=$((total + NEG_CASES))
fi

echo "# $((total - fails))/$total kiểm sạch (boot=$N_BOOT, tests=$EXPECT_TESTS)"
echo "# infra: guard_log=$QG_LOG qemu_own=$(qg_own)"
[ "$fails" -eq 0 ] || exit 1
exit 0
