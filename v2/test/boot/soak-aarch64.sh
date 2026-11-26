#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-265 — soak boot ảnh **aarch64** K4' (điều kiện "**như K3**" của K4': soak 1.000).
#
# Khuôn: `soak.sh` (riscv64, KT-57). **Cùng bất biến, không nới một ngưỡng nào**:
#   - D2-24 + QD §5.3: máy `ps`, **N = 1.000**, **3 luồng song song**, `n ≥ 3/p` với p = 0,3 %
#     (⇒ n ≥ 1.000); 0 lỗi/N; 0/N chỉ là **trần**, không phải bảo đảm.
#   - D2-72 Q10: timeout **20 s/boot**, chỉ `kill` PID do chính script tạo (AGENTS §9),
#     BUILD_ID = git sha + sha256 ảnh, ghi qemu version + cấu hình; **không retry** (D2-07).
#   - D2-74 điều kiện (c): soak **KHÔNG** `-icount`.
#   - D2-80: mỗi log boot phải có **hash ảnh do kernel tự in** (`# conf image: build=<16 hex>
#     text=<len>`, FNV-1a 64 trên `.text` đã nạp); script băm lại **cùng** section từ ELF
#     (`test/conformance/image_digest.py`) và **từ chối** log không có/lệch hash.
#   - D2-08: ảnh sản phẩm KHÔNG chứa chuỗi `ktest` (kiểm trước khi soak, như `run.sh`).
#
# Bất biến của MỖI boot (fail-closed, không nới):
#   1. đúng 1 `TAP version 13`; đúng 1 `1..46`; đúng **46** dòng `ok N - `, đủ số 1..46,
#      **không** dòng `not ok` nào (KT-269 thêm 3 ca U-mode: 15 → 18; KT-273 thêm 3 ca PCI/blk:
#      18 → 21; **KT-275** thêm 4 ca mạng: 21 → 25; KT-277 thêm 2 ca W^X/cache-DMA: 25 → 27;
#      **KT-279** thêm 3 ca GPU: 27 → 30; **KT-283** thêm 2 ca bàn phím: 30 → 32;
#      **KT-289** thêm 3 ca TCP/DNS: 32 → 35);
#   2. đúng 1 `# viFastOs aarch64: boot ok` (marker **nguyên văn**, in sau khi mọi TAP xanh);
#   3. đúng 1 `# conf image: build=<16 hex> text=<len>`, build/text **khớp ELF** (D2-80);
#   4. `# smp: online mask=0xf alive=0xf expect=0xf harts=4 mpidr_ok=1` — **bốn** hart (KT-262);
#      kèm bằng chứng từng hart (`# smp: hart=N online|alive`), IPI `# smp: ipi recv=1` đúng một
#      lần, và ba dòng `# tlbi:` (read1 match=1, shootdown acks=3 ok=1, read2 fault=1);
#   5. **KT-269** — đúng **một** dòng `# umode: el0 ran svc=<n> ret=<v>` với `svc=0` (imm16 của
#      `svc #0`) và `ret > 0` (DebugPutStr trả độ dài chuỗi đã in): bằng chứng EL0 **thật sự** chạy
#      và `svc` được xử lý. Ảnh chỉ chạy EL1 (không `eret` xuống EL0) **không** có dòng này ⇒ ĐỎ
#      dù TAP 1..21 vẫn xanh;
#   6. **KT-275** — bốn dòng bằng chứng mạng: `# net: device … vendor=0x1af4 … mac=<6 octet> …
#      assigned=… readback=…`, `# net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0
#      server=10.0.2.2`, `# net: arp gw=<mac>`, `# net: ping 10.0.2.2 ok n=3 rtt=<ms>` — mỗi dòng
#      **đúng một** lần. Ảnh mất virtio-net/DHCP/ARP/ICMP ⇒ soak ĐỎ dù TAP 1..21 vẫn xanh;
#   7. **không** dòng `PANIC` nào (tính cả dòng thụt đầu dòng — fail-closed);
#   8. quá **20 s** ⇒ `kill` đúng PID, boot đó **đỏ** (không retry).
#   5c. **KT-277** — đúng **một** dòng `# wx: … text=RX data=RW ok=1 … fault_ec=0x25 …` và đúng
#      **một** dòng `# dma: cache-clean ok=1 lines=<n> bytes=<m> …` với `n > 0`, `m > 0` và
#      `n × line == m`. Ảnh map `.text` RW (không fault) hoặc bỏ `dc civac` (`lines=0`) ⇒ ĐỎ;
#   5d. **KT-279** — mười dòng bằng chứng GPU (`# gpu pci:`/`dev:`/`dma:`/`fb:`/`display:`/
#      `resource:`/`scanout:`/`draw:`/`console:`/`stats:`) + hai marker (`# gpu: drawn`,
#      `# gpu: console done lines=3`), mỗi dòng **đúng một** lần; `# gpu fb:` phải có
#      `identity=1 bytes=1228800 pages=300 pa=0x42000000 walk=0x42000000` (framebuffer **ngoài
#      ảnh** — trần I7 **không** bị nới), `# gpu scanout:` phải có `bogus=InvalidResourceId`,
#      `# gpu console:` phải có `commands=2 matched=29/29`, `# gpu stats:` phải có
#      `errors=0 timeouts=0 id_mismatch=0 rejects=1` và `fb_clean_lines > 0`. Ảnh mất GPU ⇒ ĐỎ;
#
#   5e. **KT-283** — tám dòng bằng chứng bàn phím (`# input pci:`/`cap:`/`name:`/`dev:`/`dma:`/
#      `# input: ready`/`# input: key a `/`# input stats:`), mỗi dòng **đúng một** lần; `# input
#      pci:` phải có `vendor=0x1af4 device=0x1052 assigned=0x10600000` (leaf **riêng** của input),
#      `# input name:` phải là `QEMU Virtio Keyboard` (đọc từ thiết bị), `# input dev:` phải có
#      `version1=1 rep_feature=0 device_status=0x0f num_queues=2 queue_size=16
#      status_queue_size=16 key_a_bit=1`, `# input dma:` phải có `identity=1 armed=8
#      status_queue_idle=1`, `# input: key a ` phải có `code=30 ascii=0x61 shift=0`, và `# input
#      stats:` phải có `errors=0 timeouts=0 id_mismatch=0` + `clean_lines > 0` + `inval_lines > 0`.
#      Ảnh mất bàn phím ⇒ soak ĐỎ dù TAP 1..30 vẫn xanh.
#
# **KT-283 — bàn phím có mặt trong soak, ghi rõ lựa chọn**: `QEMU_ARGS` **giữ**
# `-device virtio-keyboard-pci` và mỗi boot mở **HMP monitor** trên unix socket riêng rồi bơm
# `sendkey a` **sau** dòng `# input: ready` của khách (cùng kỹ thuật `run-aarch64.sh`/`input.sh`
# riscv64). Lựa chọn thay thế **bị loại**: (a) bỏ `-device virtio-keyboard-pci` ⇒ ảnh in
# `not ok 31/32` ⇒ soak ĐỎ, tức phải nới bất biến TAP — không được phép; (b) thêm cờ "chế độ soak"
# vào ảnh để bỏ bước input ⇒ ảnh sản phẩm có nhánh chỉ dùng cho test (D2-08 cấm). Chi phí **đo
# được**: mỗi boot thêm một socket + một lệnh HMP (~50 ms), 1.000 boot ÷ 3 luồng ≈ **+17 s** cho
# toàn soak — rẻ hơn 500 ms giữ khung hình của GPU (KT-279).
#
# **KT-348b — thứ tự gửi phím (sửa lỗi harness đo được trong KT-348, KHÔNG nới ngưỡng)**: monitor
# được nối **trước** (helper `lib/monitor-sendkey.py` khởi động song song với lúc khách boot) và
# `sendkey a` được gửi trên kết nối **đã mở sẵn** ngay khi thấy `# input: ready` (dòng này in **sau**
# khi ảnh đã arm 8 đệm). Đường cũ — chờ ready rồi mới spawn `python3` + `connect` — nằm trong cửa sổ
# 5 s của ảnh; dưới tải máy đo đo được 1.578–3.827 ms và boot 861 chạm 5.076 ms ⇒ ảnh hết hạn,
# QEMU thoát, `connect` nhận `[Errno 2]`, lỗi bị `|| true` nuốt nên trông như lỗi nhân. Nay lỗi gửi
# phím là `fail i monitor-<lý do>` **có tên** (T-INFRA), và mọi tệp log thiếu bị **đếm** + báo ĐỎ
# kèm lý do (xem `soak_a64_log_inventory`). Cửa sổ 5 s của ảnh và `READY_DS=100` giữ nguyên.
#
# **KT-279 — GPU có mặt trong soak, ghi rõ lựa chọn**: `QEMU_ARGS` **giữ** `-device virtio-gpu-pci`
# (cùng topology với `run-aarch64.sh`), nên mỗi boot soak chạy **cả** đường GPU. Hệ quả **đo được**:
# ảnh giữ khung hình `gpu::GPU_HOLD_MS` = **500 ms** sau marker console để runner kịp `screendump`
# (soak **không** chụp, nhưng dùng **cùng** ảnh — không có cờ "chế độ soak" trong ảnh sản phẩm,
# D2-08). Chi phí: 1.000 boot × 0,5 s ÷ 3 luồng ≈ **+2,8 phút** cho toàn soak. Lựa chọn thay thế
# (bỏ `-device virtio-gpu-pci` trong soak) **bị loại**: ảnh sẽ in `not ok 28..30` ⇒ soak ĐỎ, tức
# phải nới bất biến TAP — không được phép.
#
# **KT-289 — TCP/DNS có mặt trong soak, ghi rõ lựa chọn**: ảnh in TAP 33/34 cho **TCP client**
# (khuôn `net.sh` riscv64 KT-268), nên mỗi boot soak cần **server TCP thật** trên host. Soak chạy
# **3 luồng song song** ⇒ dùng **một** server `python3` nhận **nhiều** kết nối (mỗi boot một kết
# nối, xử lý tuần tự), khởi động **trước** boot dò và dừng sau khi mọi luồng xong; trap `EXIT`
# dọn server (chuỗi với `qg_cleanup` của `qemu_guard.sh`). Lựa chọn thay thế **bị loại**: (a) bỏ
# TCP khỏi ảnh ⇒ `not ok 33/34` ⇒ soak ĐỎ, tức phải nới bất biến TAP; (b) cờ "chế độ soak" trong
# ảnh ⇒ nhánh chỉ dùng cho test (D2-08 cấm). Chi phí **đo được**: một kết nối TCP + DNS trên mỗi
# boot (~10 ms dưới TCG) — không đáng kể so với 500 ms giữ khung hình của GPU.
#
# **KT-300 — DNS cục bộ cho soak, ghi rõ khảo sát + lựa chọn**: soak vòng 16 đạt **992/1000**,
# **8 boot đỏ đều** là `# net dns: elapsed_ms=6000 attempts=2 err=Timeout` trong khi TCP cùng boot
# xanh (D2-217) ⇒ nút thắt là **resolver của host** dưới 3 QEMU song song, không phải kernel.
# Khảo sát QEMU 11.1.3 + libslirp 4.9.7 (đo trên máy này, xem journal `kt300-dns-local.md`):
#   - `-netdev user,dns=<ip>` chỉ đặt **địa chỉ DNS ảo** (DHCP option 6 + địa chỉ slirp chặn ở
#     cổng 53); nó **không** đổi resolver thượng nguồn. slirp luôn chuyển truy vấn DNS tới
#     `get_dns_addr()` = resolver của host (`res_ninit`/`res_getservers` trên macOS).
#   - `10.0.2.2:53` (gateway) **cũng** bị chặn làm DNS (đường tương thích VirtualBox) ⇒ cũng đi
#     resolver host; chỉ `10.0.2.2:<cổng khác 53>` mới map về loopback host.
#   - `guestfwd` của QEMU **chỉ TCP** (`slirp_guestfwd`: `if (strcmp(buf, "tcp") ...) fail`);
#     `hostfwd` là chiều host → guest ⇒ **không** có đường QEMU-native cho UDP guest → host.
#   ⇒ Phương án "`dns=10.0.2.2` + `guestfwd`/`hostfwd`" **bất khả thi**. Giữ nguyên **ảnh** (T2
#     vẫn đòi DNS thật qua slirp → resolver host) và **chỉ trong soak** chuyển resolver thượng
#     nguồn của **tiến trình QEMU** về server DNS cục bộ: dylib nhỏ `DYLD_INTERPOSE`
#     `res_9_ninit`/`res_9_getservers`/`res_9_ndestroy` trả `127.0.0.1:53`; server `python3` UDP
#     bind `0.0.0.0:53` (macOS cho phép **không cần root**; `127.0.0.1:53` bị EACCES) trả A cố
#     định `192.0.2.123` TTL 3600 cho `example.com`. QEMU chạy qua **wrapper** vì `perl` của
#     `qemu_guard.sh` là binary SIP nên dyld **xoá** `DYLD_*` khỏi môi trường trước `exec` —
#     wrapper đặt lại biến rồi `exec` QEMU thật. **Không** đổi ảnh, **không** đổi
#     `run-aarch64.sh`, **không** đổi ngưỡng, **không** retry (D2-07).
#   - Bằng chứng **fail-closed**: log server phải có ≥ N+1 truy vấn (probe + N boot) và ≥ N+1 câu
#     trả lời; mỗi log boot phải có `# net: dns example.com a=192.0.2.123 ttl=3600` (TEST-NET-1
#     **không** thể do resolver thật trả) ⇒ interpose hỏng là soak ĐỎ, **không** âm thầm quay
#     lại resolver host.
#   - Đột biến (`SOAK_DNS_MUTATION`, **chỉ** để kiểm chứng cơ chế, không phải cổng): `off` =
#     không dựng server ⇒ DNS timeout ⇒ ĐỎ; `nxdomain` = server trả NXDOMAIN ⇒ TAP 35 ĐỎ;
#     `wrongip` = server trả `203.0.113.9` ⇒ log boot không có `a=192.0.2.123` ⇒ ĐỎ.
#   6. **không** dòng `PANIC` nào (tính cả dòng thụt đầu dòng — fail-closed);
#   7. quá **20 s** ⇒ `kill` đúng PID, boot đó **đỏ** (không retry).
#
# Khác `soak.sh` riscv64 **chỉ ở phần danh tính ảnh**: ảnh aarch64 **chưa** in dòng `# build:`
# của KT-86 (nợ ghi ở `v2/docs/k4-aarch64-boot.md` §7), nên cổng danh tính ở đây là
# `# conf image:` + sha256 ảnh + `git HEAD` ghi trong header. **Không** nới bất biến nào ở trên.
#
# Dùng:
#   bash v2/test/boot/soak-aarch64.sh [NBOOT] [LANES]     # mặc định 1000 3 (đúng cổng K4')
#   bash v2/test/boot/soak-aarch64.sh --smoke N [LANES]   # N < 1.000: chỉ để đỏ trước/kiểm marker;
#                                                         # LUÔN exit 1 (không phải bằng chứng cổng)
#   bash v2/test/boot/soak-aarch64.sh --selftest          # kiểm bộ đọc log + 2 lỗi harness
#                                                         # (log/QEMU, sendkey) trên log tổng hợp:
#                                                         # 1 ca dương + 45 ca âm, KHÔNG mở QEMU
#   bash v2/test/boot/soak-aarch64.sh --help
#
# Biến môi trường: `SOAK_AARCH64_LOGDIR`/`KT_LOGDIR` (mặc định `$V2_RUN_TMP` hoặc
#   `$TMPDIR/k4-aarch64-soak.<pid>`).
#
# Exit: 0 = 0 lỗi/N (chỉ ở chế độ cổng, N ≥ 1.000); 1 = có boot đỏ / ảnh đổi giữa soak;
#       2 = lỗi cấu hình (thiếu tool, N < 1.000, LANES ngoài [1,3], build/budget hỏng);
#       3 = TỪ CHỐI ĐO (ảnh không in `# conf image:` khớp ELF) — 0 boot được đếm.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
# **KT-333d (D2-248 mục 2b)** — LOGDIR (và SOCK ngắn) dùng chung `v2/test/boot/lib/env.sh`
# (KT-333c đã làm logdir theo phiên inline ở 6 runner; nay gom một chỗ, không lặp code).
. "$HERE/lib/env.sh"
# **KT-347b** — vòng soak/tổng hợp/BUILD_ID/kết luận + quét marker D2-08 ở **một** nguồn
# (`lib/soak-common.sh`, kéo theo `lib/harness-scan.sh`); runner giữ `check_boot_log`/QEMU/DNS/TCP.
# shellcheck source=/dev/null
. "$HERE/lib/soak-common.sh"
# **KT-407** — helper dùng chung: `rc=126` + `log_inventory` + **một** `trap EXIT` (nợ KT-348b).
# shellcheck source=/dev/null
. "$HERE/lib/run-common.sh"
# **KT-408e** (D2-263) — **SSOT số ca TAP**: `tap_parity_check` khẳng định
# `soak.TAP_TESTS == run.EXPECT_TESTS == kbin.TAP_TESTS` cho **cả** `aarch64` lẫn `x86_64` bằng
# cách đọc **mã thật**; gọi trong `--selftest` (chạy mọi T2, 0 bước cổng mới) kèm ca âm lệch ±1
# ⇒ lệch TAP bị bắt trong ~0,3 s thay vì đốt 1.000 boot như Peak 2 (`becc1756`).
# shellcheck source=/dev/null
. "$HERE/lib/tap-parity.sh"
REPO_ROOT="$(cd "$V2_ROOT/.." && pwd)"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
BUDGET="$HERE/budget-aarch64.sh"
DIGEST="$V2_ROOT/test/conformance/image_digest.py"
TARGET="aarch64-unknown-none"
# **KT-407** — tôn trọng `CARGO_TARGET_DIR` (cùng quy ước cargo; xem `run-aarch64.sh`).
IMG_DIR="${CARGO_TARGET_DIR:-$V2_ROOT/kernel/target}/$TARGET/release"
IMAGE="$IMG_DIR/vifast-kbin-aarch64"
# **KT-338c** (D2-08) — soak boot **ảnh kiểm** (`--features ktest`, có TAP/dòng bằng chứng) và
# kiểm riêng **ảnh sản phẩm** (không feature) trước khi vào vòng soak.
KTEST_IMAGE_PROD_COPY=""
PRODUCT_IMAGE=""
# **KT-333c** (D2-243 mục 3b) — logdir **riêng theo phiên**: `$V2_RUN_TMP` hoặc
# `$TMPDIR/k4-aarch64-soak.<pid>`; ép bằng `KT_LOGDIR`/`SOAK_AARCH64_LOGDIR`.
LOGDIR="$(v2_env_logdir k4-aarch64-soak SOAK_AARCH64_LOGDIR)"
if [ "${1:-}" = "--print-logdir" ]; then
    echo "$LOGDIR"
    exit 0
fi
QEMU_MIN_MAJOR=9 # cùng sàn với riscv64 (D2-72 Q2): QEMU >= 9.2
QEMU_MIN_MINOR=2
TIMEOUT_DS=200 # 20 s/boot (D2-72 Q10), đơn vị 0,1 s
# **KT-283**: số bước 0,05 s chờ dòng `# input: ready` của khách trước khi coi là ảnh không lên
# được (100 × 0,05 s = 5 s — cùng ngân sách `INPUT_WAIT_SECS` của ảnh).
READY_DS=100
P_NUM=3        # p = 0,3 % = 3/1000 (D2-24)
P_DEN=1000
TAP_TESTS=50   # **KT-408e** (D2-263) — khớp `1..50` của ảnh
               # (`vifast-kbin-aarch64/src/main.rs::TAP_TESTS` = 50) và `EXPECT_TESTS` của
               # `run-aarch64.sh`; số này bị `lib/tap-parity.sh` khoá chéo `soak==run==kbin`
               # nên **không** phải nguồn thứ hai. Lịch sử: **KT-338b** 35 + 6 ca core (36..41);
               # **KT-339** +3 ca K3.5 (42..44); KT-339a +45/46 (shell sản phẩm) = 46 — dừng ở
               # đây suốt Peak 2 nên soak ĐỎ 1.000/1.000 (lệch 46 ↔ 50, `D2-263`). **KT-412**
               # thêm 47..49 (`smp_suite::run`: `smp harts bootinfo`, `smp 4 hart preemptive`,
               # `smp published rule`) + dòng `# smp core:`; **KT-418** thêm 50
               # (`pan: el1 khong doc trang el0` + dòng `# pan:`).
# **KT-273** — hex 16 byte đầu của sector 0. Giá trị dưới đây là của chữ ký `VIFASTOS-KT273-S0`
# (dùng cho `--selftest`, không có đĩa thật); đường chạy thật **ghi đè** bằng `od` trên đĩa tạm.
WANT_SECTOR0_HEX=5649464153544f532d4b543237332d53
SMOKE=0
SELFTEST=0

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

die() {
    echo "soak-aarch64.sh: $*" >&2
    exit 2
}

usage() {
    sed -n 's/^# \{0,1\}//; 2,47p' "${BASH_SOURCE[0]}" 2>/dev/null | sed -n '1,60p'
    echo "soak-aarch64.sh: dùng: soak-aarch64.sh [NBOOT] [LANES] | soak-aarch64.sh --smoke N [LANES] | soak-aarch64.sh --selftest | soak-aarch64.sh --help"
}

# ------------------------------------------------------------------ bộ đọc log (một boot)
# `check_boot_log <log> <want_build> <want_text>`: 0 ⇔ log đạt **mọi** bất biến ở đầu tệp.
# In lý do đỏ ra stdout (dòng `#`). Dùng **cùng** hàm cho soak thật và cho `--selftest` ⇒ không có
# đường thứ hai lệch nhau về kỳ vọng.
check_boot_log() {
    local log="$1" want_build="$2" want_text="$3" miss_reason="${4:-}" bad=0 n got_build got_text i line

    [ -f "$log" ] || {
        # **KT-348b** — thiếu tệp log phải nêu **lý do** (không im lặng như 7 boot 814..823 của
        # KT-348). `$4` do `boot_one` truyền (mã QEMU/đường khởi động); không có thì ghi rõ là
        # "không rõ" — vẫn là ĐỎ, không bao giờ coi im lặng là xanh.
        echo "# không có log $log (lý do: ${miss_reason:-không rõ — harness không ghi được log})"
        return 1
    }

    # (1) khung TAP — đúng **một** dòng mỗi mẫu, đủ số 1..21, không `not ok`.
    n="$(grep -c -x -F 'TAP version 13' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# 'TAP version 13' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -c -x -F "1..$TAP_TESTS" "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# plan '1..$TAP_TESTS' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -c '^ok [0-9][0-9]* - ' "$log" || true)"
    [ "${n:-0}" -eq "$TAP_TESTS" ] || {
        echo "# có ${n:-0} dòng 'ok', cần $TAP_TESTS"
        bad=1
    }
    n="$(grep -c '^not ok ' "$log" || true)"
    [ "${n:-0}" -eq 0 ] || {
        echo "# có ${n:-0} dòng 'not ok'"
        bad=1
    }
    i=1
    while [ "$i" -le "$TAP_TESTS" ]; do
        grep -q "^ok $i - " "$log" || {
            echo "# thiếu 'ok $i - ...'"
            bad=1
        }
        i=$((i + 1))
    done

    # (2) marker **nguyên văn**, đúng **một** lần (0 = ảnh không báo boot; 2 = in lặp).
    n="$(grep -Fxc '# viFastOs aarch64: boot ok' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# marker '# viFastOs aarch64: boot ok' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }

    # (3) D2-80 — hash ảnh do kernel tự in, khớp ELF đang chạy.
    n="$(grep -cE '^# conf image: build=[0-9a-f]{16} text=[0-9]+$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# conf image:' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    got_build="$(sed -n 's/^# conf image: build=\([0-9a-f]*\) text=.*/\1/p' "$log" | head -1)"
    got_text="$(sed -n 's/^# conf image: build=.* text=\([0-9]*\)$/\1/p' "$log" | head -1)"
    [ "$got_build" = "$want_build" ] || {
        echo "# image-build=${got_build:-none} lệch ELF ($want_build)"
        bad=1
    }
    [ "$got_text" = "$want_text" ] || {
        echo "# image-text=${got_text:-none} lệch ELF ($want_text)"
        bad=1
    }

    # (4) bốn hart + bằng chứng từng hart + ba dòng `# tlbi:` (cùng bộ với `run-aarch64.sh`).
    grep -Eq '^# smp: online mask=0xf alive=0xf expect=0xf harts=4 mpidr_ok=1 ' "$log" || {
        echo "# thiếu '# smp: online mask=0xf alive=0xf expect=0xf harts=4 mpidr_ok=1'"
        bad=1
    }
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
        [ "${n:-0}" -eq 1 ] || {
            echo "# dòng '$line' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        }
    done
    for line in \
        "# tlbi: hart=1 read1=" \
        "# tlbi: shootdown " \
        "# tlbi: hart=1 read2 "; do
        n="$(grep -Fc "$line" "$log" || true)"
        [ "${n:-0}" -eq 1 ] || {
            echo "# dòng '$line' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        }
    done
    grep -F "# tlbi: hart=1 read1=" "$log" | grep -Fq "match=1" || {
        echo "# dòng read1 của hart 1 không có 'match=1' (hart 1 chưa nạp TLB)"
        bad=1
    }
    grep -F "# tlbi: shootdown " "$log" | grep -Fq "acks=3" || {
        echo "# dòng shootdown không có 'acks=3' (thiếu ack)"
        bad=1
    }
    grep -F "# tlbi: shootdown " "$log" | grep -Fq "ok=1" || {
        echo "# dòng shootdown không có 'ok=1'"
        bad=1
    }
    grep -F "# tlbi: hart=1 read2 " "$log" | grep -Fq "fault=1" || {
        echo "# dòng read2 của hart 1 không có 'fault=1' (TLB của hart 1 chưa bị xoá)"
        bad=1
    }

    # (5) KT-269 — bằng chứng U-mode: đúng **một** dòng `# umode: el0 ran svc=<n> ret=<v>`, với
    #     `svc=0` (imm16 của `svc #0`) và `ret > 0` (DebugPutStr trả độ dài chuỗi đã in). Dòng này
    #     chỉ in khi EL0 **thật sự** chạy và handler `svc` đọc được số hiệu từ `x8` của EL0; ảnh chỉ
    #     chạy EL1 (không `eret`) **không** có ⇒ ĐỎ, dù TAP 1..21 vẫn xanh. Cùng bộ kiểm với
    #     `run-aarch64.sh` (`check_umode`) — không có đường thứ hai lệch kỳ vọng.
    n="$(grep -cE '^# umode: el0 ran svc=[0-9]+ ret=[0-9]+$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# umode: el0 ran svc=<n> ret=<v>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    grep -Eq '^# umode: el0 ran svc=0 ret=[1-9][0-9]*$' "$log" || {
        echo "# dòng '# umode: el0 ran svc=0 ret=<len>' không đúng (imm16 phải 0, ret > 0)"
        bad=1
    }

    # (5b) **KT-273** — PCI ECAM + virtio-blk + sector 0. Cùng bất biến `run-aarch64.sh`
    #      (`check_pci`/`check_blk`): ECAM giải mã thật (host bridge class `0x060000`, slot trống
    #      `0xffff`), BAR gán rồi **đọc lại** đúng, và hex sector 0 khớp **16 byte đầu của đĩa
    #      tạm** với `magic=1`. Ảnh mất PCI/blk ⇒ soak ĐỎ dù TAP 1..21 vẫn xanh.
    n="$(grep -cE '^# pci: ecam ' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# pci: ecam ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    grep -E '^# pci: ecam ' "$log" | grep -Fq "class=0x060000" || {
        echo "# dòng '# pci: ecam' không có 'class=0x060000'"
        bad=1
    }
    grep -E '^# pci: ecam ' "$log" | grep -Fq "empty=0xffff" || {
        echo "# dòng '# pci: ecam' không có 'empty=0xffff'"
        bad=1
    }
    # **KT-302**: phải quét **đủ 256 bus** (ECAM 256 MiB map bằng block 2 MiB). `buses=16` là
    # hành vi cũ của KT-273 ⇒ soak ĐỎ, không được coi là "vẫn chạy".
    grep -E '^# pci: ecam ' "$log" | grep -Fq "buses=256" || {
        echo "# dòng '# pci: ecam' không có 'buses=256' (KT-302)"
        bad=1
    }
    # **KT-305**: `translate`/`unmap` phải **hiểu** block descriptor — dòng `# pci: unmap` do
    # `ecam.rs` in từ phép đo trên bảng trang thật. `blocks_split=0` (xoá cả block) ⇒ soak ĐỎ.
    n="$(grep -cE '^# pci: unmap ' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# pci: unmap ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    for want in "level=2" "translate_ok=1" "split=1" "neighbor_ok=1" "unmapped_ok=1" "remap_ok=1" "blocks_split=1"; do
        grep -E '^# pci: unmap ' "$log" | grep -Fq "$want" || {
            echo "# dòng '# pci: unmap' không có '$want' (KT-305)"
            bad=1
        }
    done
    n="$(grep -cE '^# pci: virtio-blk found ' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# pci: virtio-blk found ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    grep -E '^# pci: virtio-blk found ' "$log" | grep -Fq "vendor=0x1af4" || {
        echo "# dòng virtio-blk không có 'vendor=0x1af4'"
        bad=1
    }
    n="$(grep -cE '^# blk: sector0 hex=[0-9a-f]{32} ' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# blk: sector0 hex=…' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    grep -E '^# blk: sector0 ' "$log" | grep -Fq "hex=$WANT_SECTOR0_HEX" || {
        echo "# sector0 hex không khớp 16 byte đầu của đĩa tạm ($WANT_SECTOR0_HEX)"
        bad=1
    }
    grep -E '^# blk: sector0 ' "$log" | grep -Fq "magic=1" || {
        echo "# sector0 không có 'magic=1'"
        bad=1
    }

    # (5c) **KT-275** — virtio-net + DHCP + ARP + ICMP. Cùng bất biến `run-aarch64.sh`
    #      (`check_net`): MAC **đọc từ `virtio_net_config`** (không toàn 0), BAR gán rồi đọc lại
    #      đúng, DHCP/ARP/ping **thật** của slirp. Ảnh mất mạng ⇒ soak ĐỎ dù TAP 1..21 vẫn xanh.
    n="$(grep -cE '^# net: device ' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# net: device ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    grep -E '^# net: device ' "$log" | grep -Fq "vendor=0x1af4" || {
        echo "# dòng '# net: device' không có 'vendor=0x1af4'"
        bad=1
    }
    grep -E '^# net: device ' "$log" | grep -Eq 'mac=([0-9a-f]{2}:){5}[0-9a-f]{2}( |$)' || {
        echo "# dòng '# net: device' không có 'mac=<6 octet>'"
        bad=1
    }
    grep -E '^# net: device ' "$log" | grep -Eq 'mac=(00:){5}00( |$)' && {
        echo "# MAC toàn 0 — không phải MAC đọc từ thiết bị"
        bad=1
    }
    n="$(grep -cE '^# net: dhcp ip=10\.0\.2\.15 gw=10\.0\.2\.2 mask=255\.255\.255\.0 server=10\.0\.2\.2$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# net: arp gw=[0-9a-f]{2}(:[0-9a-f]{2}){5}$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# net: arp gw=<mac>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    # MAC toàn 0 / broadcast là giá trị **vô nghĩa** cho một host (cùng bất biến `arp_parse_reply`).
    grep -E '^# net: arp gw=' "$log" | grep -Eq 'gw=(00:){5}00$' && {
        echo "# ARP trả MAC toàn 0 — không phải reply hợp lệ"
        bad=1
    }
    grep -E '^# net: arp gw=' "$log" | grep -Eq 'gw=(ff:){5}ff$' && {
        echo "# ARP trả MAC broadcast — không phải reply hợp lệ"
        bad=1
    }
    n="$(grep -cE '^# net: ping 10\.0\.2\.2 ok n=3 rtt=[0-9]+$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# net: ping 10.0.2.2 ok n=3 rtt=<ms>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    # (5c2) **KT-289** — TCP client + DNS. Cùng bất biến `run-aarch64.sh` (`check_net`): bắt tay
    #       ba bước **thật** với server trên host (`# net: tcp connect … state=established`), nhận
    #       **đúng** chuỗi server gửi (`# net: tcp recv=13 first=vifast-tcp-ok`), gửi lại + đóng
    #       FIN (`# net tcp data: … send=ok close=ok`), và DNS **thật** của slirp
    #       (`# net: dns example.com a=<ip> ttl=<n>`). Ảnh mất TCP/DNS ⇒ soak ĐỎ dù TAP 1..32
    #       vẫn xanh. Dòng `# net: no tcp`/`# net: no dns` là dấu hiệu **thất bại** — có nó là ĐỎ.
    n="$(grep -cE '^# net: tcp connect 10\.0\.2\.2:45678 ok rtt=[0-9]+ state=established$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# net: tcp connect 10.0.2.2:45678 ok rtt=<ms> state=established' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# net: tcp recv=13 first=vifast-tcp-ok$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# net: tcp recv=13 first=vifast-tcp-ok' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# net tcp data: recv=13 sent=15 send=ok close=ok state=(fin-wait-2|time-wait|closing|last-ack|closed) retransmits=[0-9]+ elapsed_ms=[0-9]+ const:timeout_ms=3000$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# net tcp data:' (recv=13 sent=15 send=ok close=ok) xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    n="$(grep -cE '^# net: dns example\.com a=[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+ ttl=[0-9]+$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# net: dns example.com a=<ip> ttl=<n>' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    # **KT-294** — dòng `# net dns:` phải có `attempts=1|2` (cùng bất biến `run-aarch64.sh`):
    # `attempts=2` = lần đầu timeout rồi gửi lại; `attempts=3` = retransmit vô hạn ⇒ ĐỎ.
    n="$(grep -cE '^# net dns: name=example\.com id=0x4b55 rcode=0 an=1 reply_len=[0-9]+ rtt_ms=[0-9]+ elapsed_ms=[0-9]+ attempts=(1|2) const:timeout_ms=3000$' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# net dns: … attempts=(1|2) const:timeout_ms=3000' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    for want in '# net: no tcp' '# net: no dns'; do
        n="$(grep -Fc "$want" "$log" || true)"
        [ "${n:-0}" -eq 0 ] || {
            echo "# log có '$want' (bước TCP/DNS thất bại)"
            bad=1
        }
    done
    # (5c) **KT-277** — W^X toàn ảnh + cache maintenance trước DMA. Cùng bất biến `run-aarch64.sh`
    #      (`check_wx`/`check_dma`): quyền đọc lại **từ bảng trang** (`text=RX data=RW ok=1`), một
    #      lần **ghi** vào `.text` từ EL1 phải sinh data abort quyền (`fault_ec=0x25`,
    #      `fault_access=Write`), `.text` **không đổi** (`text_unchanged=1`), `.data` ghi được
    #      (`data_rw=1`); và `dc civac`/`dc ivac` đã chạy thật (`lines>0`, `inval_lines>0`,
    #      `lines × line == bytes`). Ảnh map `.text` RW hoặc bỏ `dc` ⇒ soak ĐỎ dù TAP 1..21 xanh.
    n="$(grep -cE '^# wx: ' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# wx: ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    for want in "text=RX" "data=RW" "ok=1" "fault_access=Write" "text_unchanged=1" "data_rw=1"; do
        grep -E '^# wx: ' "$log" | grep -Fq "$want" || {
            echo "# dòng '# wx:' không có '$want'"
            bad=1
        }
    done
    grep -Eq '^# wx: .*fault_ec=0x(25|21)' "$log" || {
        echo "# dòng '# wx:' không có 'fault_ec=0x25'/'0x21' (ghi vào .text KHÔNG fault)"
        bad=1
    }
    n="$(grep -cE '^# dma: ' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# dma: ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    grep -E '^# dma: ' "$log" | grep -Fq "cache-clean ok=1" || {
        echo "# dòng '# dma:' không có 'cache-clean ok=1'"
        bad=1
    }
    dma_line="$(grep -E '^# dma: ' "$log" || true)"
    dma_lines="$(printf '%s\n' "$dma_line" | sed -n 's/.* lines=\([0-9][0-9]*\).*/\1/p')"
    dma_bytes="$(printf '%s\n' "$dma_line" | sed -n 's/.* bytes=\([0-9][0-9]*\).*/\1/p')"
    dma_line_bytes="$(printf '%s\n' "$dma_line" | sed -n 's/.* line=\([0-9][0-9]*\).*/\1/p')"
    dma_inval="$(printf '%s\n' "$dma_line" | sed -n 's/.* inval_lines=\([0-9][0-9]*\).*/\1/p')"
    if [ -z "$dma_lines" ] || [ "$dma_lines" -le 0 ]; then
        echo "# dòng '# dma:' có lines=${dma_lines:-none} — không có lệnh 'dc civac' nào chạy"
        bad=1
    fi
    if [ -z "$dma_inval" ] || [ "$dma_inval" -le 0 ]; then
        echo "# dòng '# dma:' có inval_lines=${dma_inval:-none} — không có lệnh 'dc ivac' nào chạy"
        bad=1
    fi
    if [ -n "$dma_lines" ] && [ -n "$dma_bytes" ] && [ -n "$dma_line_bytes" ] &&
        [ "$dma_line_bytes" -gt 0 ] && [ $((dma_lines * dma_line_bytes)) -ne "$dma_bytes" ]; then
        echo "# dòng '# dma:' lines×line=$((dma_lines * dma_line_bytes)) khác bytes=$dma_bytes"
        bad=1
    fi

    # (5d) **KT-279** — virtio-gpu 2D + console trên framebuffer. Cùng bất biến `run-aarch64.sh`
    #      (`check_gpu`): BAR gán rồi **đọc lại** đúng ở leaf riêng của GPU (`0x10800000`), feature
    #      VERSION_1 + `device_status=0x0f` + `num_scanouts=1`, framebuffer **ngoài ảnh** map
    #      identity (`pa=0x42000000 walk=0x42000000 identity=1 bytes=1228800 pages=300`), ca âm đối
    #      chứng `bogus=InvalidResourceId`, console flush **một lô** (`commands=2`, `matched=29/29`),
    #      và cache maintenance framebuffer **đã chạy** (`fb_clean_lines>0`). Ảnh mất GPU ⇒ soak ĐỎ
    #      dù TAP 1..27 vẫn xanh.
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
        [ "${n:-0}" -eq 1 ] || {
            echo "# dòng khớp '$want' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        }
    done
    for want in '^# gpu: drawn$' '^# gpu: console done lines=3$'; do
        n="$(grep -cE "$want" "$log" || true)"
        [ "${n:-0}" -eq 1 ] || {
            echo "# dòng khớp '$want' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        }
    done
    gpu_pci="$(grep -E '^# gpu pci: ' "$log" || true)"
    for want in "vendor=0x1af4" "device=0x1050" "assigned=0x10800000"; do
        case "$gpu_pci" in
            *"$want"*) ;;
            *)
                echo "# dòng '# gpu pci:' không có '$want'"
                bad=1
                ;;
        esac
    done
    gpu_dev="$(grep -E '^# gpu dev: ' "$log" || true)"
    for want in "version1=1" "device_status=0x0f" "num_scanouts=1"; do
        case "$gpu_dev" in
            *"$want"*) ;;
            *)
                echo "# dòng '# gpu dev:' không có '$want'"
                bad=1
                ;;
        esac
    done
    gpu_fb="$(grep -E '^# gpu fb: ' "$log" || true)"
    for want in "identity=1" "bytes=1228800" "pages=300" "pa=0x42000000" "walk=0x42000000"; do
        case "$gpu_fb" in
            *"$want"*) ;;
            *)
                echo "# dòng '# gpu fb:' không có '$want'"
                bad=1
                ;;
        esac
    done
    gpu_scanout="$(grep -E '^# gpu scanout: ' "$log" || true)"
    case "$gpu_scanout" in
        *"bogus=InvalidResourceId"*) ;;
        *)
            echo "# dòng '# gpu scanout:' không có 'bogus=InvalidResourceId' (thiết bị không kiểm resource_id)"
            bad=1
            ;;
    esac
    gpu_console="$(grep -E '^# gpu console: ' "$log" || true)"
    for want in "chars=29" "newlines=3" "scrolls=0" "dirty=8,8,88,24" "flushes=1" "commands=2" \
        "matched=29/29" "text=ViFastOs|gpu console|0123456789"; do
        case "$gpu_console" in
            *"$want"*) ;;
            *)
                echo "# dòng '# gpu console:' không có '$want'"
                bad=1
                ;;
        esac
    done
    gpu_stats="$(grep -E '^# gpu stats: ' "$log" || true)"
    for want in "errors=0" "timeouts=0" "id_mismatch=0" "rejects=1"; do
        case "$gpu_stats" in
            *"$want"*) ;;
            *)
                echo "# dòng '# gpu stats:' không có '$want'"
                bad=1
                ;;
        esac
    done
    gpu_clean="$(printf '%s\n' "$gpu_stats" | sed -n 's/.* fb_clean_lines=\([0-9][0-9]*\).*/\1/p')"
    if [ -z "$gpu_clean" ] || [ "$gpu_clean" -le 0 ]; then
        echo "# dòng '# gpu stats:' có fb_clean_lines=${gpu_clean:-none} — không có lệnh 'dc civac' nào chạy trên framebuffer"
        bad=1
    fi

    # (5e) **KT-283** — virtio-input (bàn phím). Cùng bất biến `run-aarch64.sh` (`check_input`):
    #      BAR gán rồi **đọc lại** đúng ở leaf riêng của input (`0x10600000`), tên **đọc từ**
    #      `virtio_input_config`, feature VERSION_1 + **không** `REP` + `device_status=0x0f` +
    #      **hai** queue `event`/`status` + bitmap `EV_BITS` bật `KEY_A`, trang DMA map identity
    #      (`identity=1`) với 8 đệm arm sẵn, `sendkey a` ⇒ `code=30 ascii=0x61 shift=0`, và cache
    #      maintenance **đã chạy** (`clean_lines>0`, `inval_lines>0`). Ảnh mất bàn phím ⇒ soak ĐỎ
    #      dù TAP 1..30 vẫn xanh.
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
        [ "${n:-0}" -eq 1 ] || {
            echo "# dòng khớp '$want' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        }
    done
    input_pci="$(grep -E '^# input pci: ' "$log" || true)"
    for want in "vendor=0x1af4" "device=0x1052" "assigned=0x10600000"; do
        case "$input_pci" in
            *"$want"*) ;;
            *)
                echo "# dòng '# input pci:' không có '$want'"
                bad=1
                ;;
        esac
    done
    input_assigned="$(printf '%s\n' "$input_pci" | sed -n 's/.* assigned=\(0x[0-9a-f]*\).*/\1/p')"
    input_readback="$(printf '%s\n' "$input_pci" | sed -n 's/.* readback=\(0x[0-9a-f]*\).*/\1/p')"
    if [ -z "$input_assigned" ] || [ "$input_assigned" != "$input_readback" ]; then
        echo "# BAR input đọc lại (${input_readback:-none}) khác địa chỉ đã gán (${input_assigned:-none})"
        bad=1
    fi
    input_name="$(grep -E '^# input name: ' "$log" || true)"
    if [ "$input_name" != "# input name: QEMU Virtio Keyboard" ]; then
        echo "# tên thiết bị ('${input_name:-none}') không phải 'QEMU Virtio Keyboard' (đọc từ thiết bị)"
        bad=1
    fi
    input_dev="$(grep -E '^# input dev: ' "$log" || true)"
    for want in "version1=1" "rep_feature=0" "device_status=0x0f" "num_queues=2" \
        "queue_size=16" "status_queue_size=16" "key_a_bit=1"; do
        case "$input_dev" in
            *"$want"*) ;;
            *)
                echo "# dòng '# input dev:' không có '$want'"
                bad=1
                ;;
        esac
    done
    input_dma="$(grep -E '^# input dma: ' "$log" || true)"
    for want in "identity=1" "armed=8" "status_queue_idle=1"; do
        case "$input_dma" in
            *"$want"*) ;;
            *)
                echo "# dòng '# input dma:' không có '$want'"
                bad=1
                ;;
        esac
    done
    input_key="$(grep -E '^# input: key a ' "$log" || true)"
    for want in "code=30" "ascii=0x61" "shift=0"; do
        case "$input_key" in
            *"$want"*) ;;
            *)
                echo "# dòng '# input: key a' không có '$want'"
                bad=1
                ;;
        esac
    done
    input_stats="$(grep -E '^# input stats: ' "$log" || true)"
    for want in "errors=0" "timeouts=0" "id_mismatch=0"; do
        case "$input_stats" in
            *"$want"*) ;;
            *)
                echo "# dòng '# input stats:' không có '$want'"
                bad=1
                ;;
        esac
    done
    input_clean="$(printf '%s\n' "$input_stats" | sed -n 's/.* clean_lines=\([0-9][0-9]*\).*/\1/p')"
    if [ -z "$input_clean" ] || [ "$input_clean" -le 0 ]; then
        echo "# dòng '# input stats:' có clean_lines=${input_clean:-none} — không có lệnh 'dc civac' nào chạy trên vùng DMA"
        bad=1
    fi
    input_inval="$(printf '%s\n' "$input_stats" | sed -n 's/.* inval_lines=\([0-9][0-9]*\).*/\1/p')"
    if [ -z "$input_inval" ] || [ "$input_inval" -le 0 ]; then
        echo "# dòng '# input stats:' có inval_lines=${input_inval:-none} — không có lệnh 'dc ivac' nào sau khi thiết bị ghi"
        bad=1
    fi

    # (5d) **KT-408e** (D2-263) — bằng chứng **bốn hart tầng core** + **PAN**, kiểm **đích danh**
    # từng dòng thay vì chỉ dựa vào phép đếm `ok` ở (1): một ảnh rơi mất pha SMP/PAN mà vẫn in đủ
    # số dòng `ok` (vd in lặp ca cũ) phải ĐỎ. Nguồn: `KT-412` `smp_suite::run` (TAP 47..49, dùng
    # chung 3 arch) + dòng tổng kết `# smp core:` của `kbin-aarch64/src/smpcore.rs`; `KT-418`
    # `pan_probe.rs` (TAP 50) + dòng `# pan:`.
    for line in \
        '# smp core: taps=3 fails=0 boot_harts=4' \
        'ok 47 - smp harts bootinfo' \
        'ok 48 - smp 4 hart preemptive' \
        'ok 49 - smp published rule' \
        'ok 50 - pan: el1 khong doc trang el0'; do
        n="$(grep -Fxc "$line" "$log" || true)"
        [ "${n:-0}" -eq 1 ] || {
            echo "# dòng '$line' xuất hiện ${n:-0} lần, cần đúng 1"
            bad=1
        }
    done
    # `# pan:` in **đúng một** lần và phải kết luận `ok=1`. Không đòi một giá trị CPU cụ thể:
    # `cortex-a72` là ARMv8.0 (`pan_supported=0`) ⇒ đường đo "không có PAN" vẫn phải chứng minh
    # EL1 **không đọc được** trang EL0; `-cpu max` (có PAN) in `pan_supported=1 … ok=1`.
    n="$(grep -cE '^# pan: ' "$log" || true)"
    [ "${n:-0}" -eq 1 ] || {
        echo "# dòng '# pan: ' xuất hiện ${n:-0} lần, cần đúng 1"
        bad=1
    }
    grep -E '^# pan: ' "$log" | grep -Fq 'ok=1' || {
        echo "# dòng '# pan:' không có 'ok=1' (chưa chứng minh EL1 không đọc được trang EL0)"
        bad=1
    }

    # (6) không PANIC nào (in cả dòng thụt đầu dòng — fail-closed).
    n="$(grep -cE '^[[:space:]]*PANIC' "$log" || true)"
    [ "${n:-0}" -eq 0 ] || {
        echo "# có ${n:-0} dòng PANIC"
        bad=1
    }
    return "$bad"
}

# `soak_a64_log_inventory <n> <dir>`: **đếm** tệp log boot theo **từng slot** — phải **có mặt** và
# **khác rỗng** (D2-257 mục 1). Thiếu/rỗng ⇒ mất bằng chứng cho slot đó ⇒ in
# `# không có log … (lý do: …)` / `# log rỗng … (lý do: …)` và trả 1. Đây là phép kiểm còn thiếu ở
# KT-348: 7 boot 814..823 **không có tệp** (đĩa đầy) mà không ai đếm, không nêu lý do.
soak_a64_log_inventory() {
    # **KT-407** — thân chuyển sang `lib/run-common.sh` (một nguồn cho mọi runner); giữ tên hàm
    # để `--selftest` và chỗ gọi cũ không đổi. Nhãn giữ nguyên `soak-aarch64.sh` trong dòng FAIL.
    RUN_COMMON_LOG_TAG="soak-aarch64.sh" run_common_log_inventory "$1" "$2"
}

# ------------------------------------------------------------------ --selftest (không QEMU)
# Log **tổng hợp** đúng khuôn ảnh in ra (chỉ các dòng bộ đọc dùng tới). Ca dương phải XANH; mỗi ca
# âm phải ĐỎ. Nếu một ca âm lại XANH thì bộ đọc là "dấu cao su" và soak đang PASS giả.
selftest() {
    local tmp fails=0 cases=0 good py="" kdir="" ldir="" i=0 hrc=0 srv="" rc_log=0 tp_from=0
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/soak-a64-selftest.XXXXXX")" || die "không tạo được thư mục tạm"
    good="$tmp/good.log"
    cat >"$good" <<'EOF'
TAP version 13
1..50
# conf image: build=0123456789abcdef text=37044
ok 1 - pl011: init sequence applied
ok 2 - vector: 16 slots, VBAR_EL1 installed
ok 3 - brk #0: ec=0x3c, elr matches instruction
ok 4 - mmu: SCTLR_EL1.M=1 after ISB
ok 5 - mmu: identity marker + non-identity probe VA
ok 6 - high: PC at high VA, translate matches PA
ok 7 - physmap: read PA via physmap window
ok 8 - vspace: map Frame 4 KiB + walk + activate + read
ok 9 - vspace: read after unmap faults with FaultKind::Vm
ok 10 - smp: hart 1 online
ok 11 - smp: ipi 0->1
ok 12 - smp: all harts report
ok 13 - tlbi: hart 1 sees unmap
ok 14 - tlbi: ack from all harts
ok 15 - smp: 4 harts online
# smp: hart=1 online
# smp: hart=2 online
# smp: hart=3 online
# smp: ipi recv=1 intid=0
# smp: hart=0 alive
# smp: hart=1 alive
# smp: hart=2 alive
# smp: hart=3 alive
# tlbi: hart=1 read1=0x5653504143450001 magic=0x5653504143450001 match=1 sent=true done=1
# tlbi: shootdown va=0x60000000 targets=0xe expected=3 acks=3 waited=6386 timeout=0 send_failed=0x0 ok=1
# tlbi: hart=1 read2 fault=1 va=0x60000000 access=Read reason=NotMapped esr=0x96000006 far=0x60000000 sent=true done=1 seen=0x3
# mmu: map kernel=0x40080000-0x40152000 pages=202 uart=1 gic=32 probe_va=0x50000000 probe_pa=0x40096000
# smp: online mask=0xf alive=0xf expect=0xf harts=4 mpidr_ok=1 mpidr=0x80000000,0x80000001,0x80000002,0x80000003
# umode: eret entry=0x70000000 sp=0x70002000 spsr=0x3c0 mode=EL0t code_va=0x70000000 code_pa=0x4008b000 code_bytes=52 stack_va=0x70001000 stack_pa=0x40097000
ok 16 - umode: el0 entered
ok 17 - umode: svc handled
# umode: el0 ran svc=0 ret=12
# umode: svc nr=12 imm=0 elr=0x70000010 ptr=0x70000028 len=12 printed=12 error=0 ret=12
ok 18 - umode: returned to el0
# umode: exit code=0 nr=14 imm=0
# umode: done
# pci: map ecam_pa=0x4010000000 ecam_va=0x80000000 ecam_blocks=128 ecam_block_level=2 ecam_block_pa=0x4010000000 ecam_block_ok=1 bar_va=0x10000000 bar_pages=512 net_bar_va=0x10400000 net_bar_pages=512 gpu_bar_va=0x10800000 gpu_bar_pages=512 input_bar_va=0x10600000 input_bar_pages=512 fb_pa=0x42000000 fb_pages=300 fb_walk=0x42000000 input_dma_va=0xffffffc08005b000 input_dma_walk=0x400db000 frames_used=33
# pci: unmap va=0x8fe01000 pa=0x401fe01000 unmap_pa=0x401fe01000 level=2 translate_ok=1 split=1 neighbor_ok=1 unmapped_ok=1 remap_ok=1 blocks_split=1
# pci: ecam pa=0x4010000000 va=0x80000000 host=1b36:0008 class=0x060000 rev=0x00 empty=0xffff functions=5 buses=256
ok 19 - pci: ecam
# pci: virtio-blk found bus=0 dev=2 func=0 vendor=0x1af4 device=0x1001 class=0x010000 rev=0x00 bar=4 kind=mem64 size=0x4000 assigned=0x10000000 readback=0x10000000 command=0x0006 caps=1 other_bars=0 reserved=6
ok 20 - pci: virtio-blk found
# blk: sector0 hex=5649464153544f532d4b543237332d53 ascii="VIFASTOS-KT273-S" magic=1 capacity=8 status=0x0f dma=0x400cc000 desc=0x400cc000 avail=0x400cc080 used=0x400cc098
ok 21 - blk: sector0
# net: device bus=0 dev=1 func=0 vendor=0x1af4 device=0x1000 bar=4 kind=mem64 size=0x4000 assigned=0x10400000 readback=0x10400000 mac=52:54:00:12:34:56 mtu=65535 status=0x0001 queues=3 rx=16 tx=16 features=0x0000010130bf8024 driver=0x0000000100000020 dma=0x400cd000
ok 22 - net: device
# net: dhcp ip=10.0.2.15 gw=10.0.2.2 mask=255.255.255.0 server=10.0.2.2
ok 23 - net: dhcp
# net: arp gw=52:55:0a:00:02:02
ok 24 - net: arp
# net: ping 10.0.2.2 ok n=3 rtt=0
ok 25 - net: ping
# net: tcp connect 10.0.2.2:45678 ok rtt=1 state=established
# net tcp connect: iss=0x4b540001 snd_una=0x4b540002 rcv_nxt=0x12345679 retransmits=0 elapsed_ms=1 const:timeout_ms=3000 const:port=45678
ok 33 - net: tcp connect
# net: tcp recv=13 first=vifast-tcp-ok
# net tcp data: recv=13 sent=15 send=ok close=ok state=fin-wait-2 retransmits=0 elapsed_ms=2 const:timeout_ms=3000
ok 34 - net: tcp data
# net: arp dns=52:55:0a:00:02:03 ip=10.0.2.3
# net dns: name=example.com id=0x4b55 rcode=0 an=1 reply_len=45 rtt_ms=1 elapsed_ms=1 attempts=1 const:timeout_ms=3000
# net: dns example.com a=93.184.216.34 ttl=3600
ok 35 - net: dns
# net stats: tx_frames=6 tx_bytes=860 rx_frames=6 rx_bytes=1466 notifies=13 errors=0 timeouts=0 id_mismatch=0
# wx: text=RX data=RW ok=1 text_bytes=77824 data_bytes=782336 guard_pages=8 guard_bytes=32768 text_va=0xffffffc080000100 data_va=0xffffffc080013000 text_perms=RX data_perms=RW fault_ec=0x25 fault_access=Write fault_va=0xffffffc080000100 fault_elr=0xffffffc080004cf4 low_fault_ec=0x25 low_fault_va=0x40080100 text_unchanged=1 data_rw=1 data_in_rw=1
ok 26 - wx: text RX (write faults), data RW
# dma: cache-clean ok=1 lines=64 bytes=4096 ops=1 inval_lines=64 inval_bytes=4096 inval_ops=1 poll_inval=1 line=64 ctr=0x000000008444c004
ok 27 - dma: cache maintenance before/after DMA
# gpu pci: bus=0 dev=3 func=0 vendor=0x1af4 device=0x1050 bar=4 kind=mem64 size=0x4000 assigned=0x10800000 readback=0x10800000 command=0x0006 caps=1 other_bars=2 reserved=6
# gpu dev: num_queues=2 queue_max=64 queue_size=16 device_features=0x0000010130000002 driver_features=0x0000000100000000 version1=1 device_status=0x0f num_scanouts=1 num_capsets=0
# gpu dma: desc=0x400d2000 avail=0x400d2100 used=0x400d2128 cmd=0x400d2200 resp=0x400d2600 dma=0x400d2000 notify_off=0 notify_addr=0x10803000 queue=0
# gpu fb: pa=0x42000000 walk=0x42000000 identity=1 bytes=1228800 pages=300 stride=2560 attach_entries=19
ok 28 - gpu: device
# gpu display: w=1280 h=800 enabled=1 scanouts=1
# gpu resource: id=1 format=1 w=640 h=480 entries=19 chunk=65536
# gpu scanout: id=0 resource=1 w=640 h=480 host_w=1280 host_h=800 enabled=1 bogus=InvalidResourceId
# gpu draw: rect=40,40,200,120 color=0xff0000 text=ViFast at=40,200 fg=0xffffff bg=0x000000 stride=2560
# gpu: drawn
ok 29 - gpu: draw
# gpu console scroll: lines=60 scrolls=1 row0=1 row_last=1 ok=1
# gpu console: origin=8,8 cols=79 rows=59 lines=3 chars=29 newlines=3 scrolls=0 dirty=8,8,88,24 flushes=1 commands=2 matched=29/29 text=ViFastOs|gpu console|0123456789
ok 30 - gpu: console
# gpu: console done lines=3
# gpu stats: commands=10 notifies=10 errors=0 timeouts=0 id_mismatch=0 rejects=1 fb_clean_lines=19344 fb_clean_bytes=1237248 fb_clean_ops=2 published=10 queue_size=16 device_status=0x0f
# input pci: bus=0 dev=4 func=0 vendor=0x1af4 device=0x1052 class=0x090000 rev=0x01 bar=4 kind=mem64 size=0x4000 assigned=0x10600000 readback=0x10600000 command=0x0006 caps=1 other_bars=3 reserved=6
# input cap: common=0x0 notify=0x3000 device=0x2000 isr=0x1000 mul=4 cap_bar=4 ecam=0x80020000 cap_ranges_ok=1 overlap_checked=1
# input name: QEMU Virtio Keyboard
# input dev: name_len=20 devids_bustype=6 devids_vendor=0x627 devids_product=0x1 devids_version=0x1 num_queues=2 queue_max=64 queue_size=16 status_queue_size=16 device_features=0x10130000000 driver_features=0x100000000 version1=1 rep_feature=0 device_status=0x0f config_generation=6 ev_bits_len=29 key_a_bit=1
# input dma: desc=0x400d6000 avail=0x400d6100 used=0x400d6128 buf=0x400d6400 dma=0x400d6000 walk=0x400d6000 identity=1 armed=8 notify_off=0 notify_addr=0x10603000 queue=0 status_notify_off=1 status_queue_idle=1
ok 31 - input: device
# input: ready
# input: key a code=30 ascii=0x61 shift=0 elapsed_ms=153
ok 32 - input: key
# input stats: events=1 syncs=0 key_downs=1 key_ups=0 notifies=2 errors=0 timeouts=0 id_mismatch=0 clean_lines=128 clean_bytes=8192 clean_ops=2 inval_lines=64 inval_ops=1 published=9 queue_size=16 device_status=0x0f
# core phase: eret entry=0x70000034 sp=0x70002000 mode=EL0t nr_syscalls=8
# core switch: nr=0 state=0x0 — điểm an toàn, kernel tiếp quản
# core resume: tcb=4 state_after_switch=0x0 entry=0x70000114 match=1
# core: addr=0xffffffc0800414a8 size=184792 pin=196608 in_image=1 harts=4 objects=8 caps=9 healthy=1
# core buffer: va=0x70002000 mr_offset=80 extra_offset=560 configure_offset=608 harts=4
# core lock: acquired=16 contended=0 in_cs=0 overlap=0
# core configure: priority_old=4 priority_now=9 err0=0 err_ro=3 err_vspace=14 err_entry=0 configure=2
# core ipc: send_err=0 len=8 mr0=0x338b0000 mr3=0x338b0003 mr4=0x338b0004 mr7=0x338b0007 server=Ready
# core suspend: switched=1 state=Inactive resume_entry=0x70000114 resume_ok=1 exit=0 deferred=1 resume_stat=1
# core lock: irq_before=1 irq_inside=0 irq_after=1 acquired=14 overlap=0 in_cs=0 ok=1
ok 36 - core: Machine linked in image, pin + invariants
ok 37 - core: Tcb.Configure priority from EL0
ok 38 - ipc: el0 send 8 MR -> endpoint recv (MR4..MR7 buffer)
ok 39 - core: Configure rights=3, vspace=14 rollback (no panic)
ok 40 - core: Tcb.Suspend(Defer) safe point + Resume entry
ok 41 - lock: DAIF.I masked in critical section, restored after
# k5 load: procsrv=0x71000000+4p entry=0x7100000c base=0x71000000 segs=3 shell=0x71200000+3p entry=0x712000d8 segs=2 echo=0x71400000+3p entry=0x7140000c segs=2 stack=0x7000a000+8192 ring_pa=0x400d5000 ring_va=0x43000000 script=27B
# k5 proc: up=1 wait_pid=66 wait_code=65535 kill_pid=67 kill_code=0 reply_wait=1 reply_kill=1 yields=3 timeout=0 takeover=1 root=9 state=Inactive
# k5 shell: exit=0 state=Inactive transcript=0xf puts=9 events_help=1 events_echo=1
# k5 echo: exit=0 state=Inactive len=8 reg0=0xe0a0 reg3=0xe0a3 buf0=0xff0000e1ff buf3=0xff0000e1fc got0=0xff0000e1ff got3=0xff0000e1fc prefill0=0xe100 mask=0xff0000ff
# k5 core: invariants=1
ok 42 - k5: procsrv serves 2 requests from EL0 (svc #0)
ok 43 - k5: shell prompt + help/echo + clean exit from EL0
ok 44 - k5: echo EL0 reads/writes IPC buffer (length + content)
# shell proc: up=1 wait_pid=16962 wait_code=65535 reply_wait=1 yields=2 timeout=0 takeover=1 state=Inactive
# shell: exit code=0 state=Inactive transcript=0xf puts=42 help=1 echo=1 ps=1 replies=1 entries=3 yields=1
# shell: invariants=1
ok 45 - shell: procsrv serves WAIT from EL0 (product path)
ok 46 - shell: product path help/echo/ps/exit over PL011 (0 marker)
# KT-412 — bốn hart tầng core (`vifast-ktest::smp_suite::run`) + tổng kết `# smp core:`.
# smp: boot_harts=4 model_harts=4 per_hart=1 idempotent=1 check_sched=1
ok 47 - smp harts bootinfo
# smp: harts=4 tcbs=8 ticks=8 double_pick=0 starved=0 sched_bad=0 published_bad=0 ran=4,4,4,4,4,4,4,4
ok 48 - smp 4 hart preemptive
# smp: published cross_read=1 owner_only=1 value_guard=1 range_guard=1
ok 49 - smp published rule
# smp core: taps=3 fails=0 boot_harts=4
# KT-418 — PAN: EL1 không đọc được trang EL0 (`cortex-a72` = ARMv8.0 ⇒ pan_supported=0).
# pan: va=0x70000000 pan_supported=0 (CPU khong co PAN — ARMv8.0) ok=1
ok 50 - pan: el1 khong doc trang el0
# viFastOs aarch64: boot ok
# viFastOs aarch64: halt
EOF
    local wb=0123456789abcdef wt=37044

    # Ca dương: log đúng khuôn **phải** XANH (nếu không, bộ đọc từ chối cả log tốt).
    cases=$((cases + 1))
    if check_boot_log "$good" "$wb" "$wt" >"$tmp/p.out" 2>&1; then
        echo "ok $cases - ca dương: log đúng khuôn ⇒ XANH"
    else
        echo "not ok $cases - ca dương: log đúng khuôn mà bộ đọc ĐỎ ($(head -1 "$tmp/p.out"))"
        fails=$((fails + 1))
    fi

    # Mỗi ca âm: biến đổi log tốt rồi đòi bộ đọc **ĐỎ**.
    neg() { # $1 = tên ca, $2 = tệp log đã biến đổi
        cases=$((cases + 1))
        if check_boot_log "$2" "$wb" "$wt" >"$tmp/n.out" 2>&1; then
            echo "not ok $cases - ca âm ($1): bộ đọc vẫn XANH"
            fails=$((fails + 1))
        else
            echo "ok $cases - ca âm ($1) ⇒ ĐỎ ($(head -1 "$tmp/n.out"))"
        fi
    }

    grep -v -x -F 'TAP version 13' "$good" >"$tmp/a.log" || true
    neg "thiếu 'TAP version 13'" "$tmp/a.log"

    sed "s/^1\.\.$TAP_TESTS\$/1..$((TAP_TESTS - 1))/" "$good" >"$tmp/b.log" || true
    neg "plan sai (1..14)" "$tmp/b.log"

    grep -v '^ok 7 - ' "$good" >"$tmp/c.log" || true
    neg "thiếu 'ok 7'" "$tmp/c.log"

    sed 's/^ok 9 - /not ok 9 - /' "$good" >"$tmp/d.log" || true
    neg "có 'not ok'" "$tmp/d.log"

    grep -v -F '# viFastOs aarch64: boot ok' "$good" >"$tmp/e.log" || true
    neg "thiếu marker" "$tmp/e.log"

    awk '{print} /^# viFastOs aarch64: boot ok$/{print}' "$good" >"$tmp/f.log" || true
    neg "marker in hai lần" "$tmp/f.log"

    sed 's/^# viFastOs aarch64: boot ok$/# viFastOs aarch64: boot ok\nPANIC: giả lập/' \
        "$good" >"$tmp/g.log" || true
    neg "có PANIC" "$tmp/g.log"

    sed 's/^# conf image: build=.*$//' "$good" >"$tmp/h.log" || true
    neg "thiếu '# conf image:'" "$tmp/h.log"

    sed 's/^# conf image: build=[0-9a-f]*/# conf image: build=ffffffffffffffff/' \
        "$good" >"$tmp/i.log" || true
    neg "build lệch ELF" "$tmp/i.log"

    sed 's/ text=37044$/ text=99999/' "$good" >"$tmp/j.log" || true
    neg "text lệch ELF" "$tmp/j.log"

    sed 's/^# smp: online mask=0xf alive=0xf expect=0xf harts=4 mpidr_ok=1 .*$/# smp: online mask=0x7 alive=0x7 expect=0x7 harts=3 mpidr_ok=1/' \
        "$good" >"$tmp/k.log" || true
    neg "chỉ 3 hart online (mask=0x7)" "$tmp/k.log"

    grep -v -F '# tlbi: shootdown ' "$good" >"$tmp/l.log" || true
    neg "thiếu dòng shootdown" "$tmp/l.log"

    sed 's/acks=3 /acks=2 /' "$good" >"$tmp/m.log" || true
    neg "shootdown thiếu ack (acks=2)" "$tmp/m.log"

    # **KT-269** — ba ca âm cho bất biến U-mode (chỉ **thêm** kiểm, không bỏ kiểm cũ).
    grep -v -F '# umode: el0 ran ' "$good" >"$tmp/n.log" || true
    neg "thiếu dòng '# umode: el0 ran …' (ảnh không xuống EL0)" "$tmp/n.log"

    sed 's/^# umode: el0 ran svc=0 ret=12$/# umode: el0 ran svc=1 ret=12/' \
        "$good" >"$tmp/o.log" || true
    neg "svc imm16 sai (svc=1 thay vì 0)" "$tmp/o.log"

    sed 's/^# umode: el0 ran svc=0 ret=12$/# umode: el0 ran svc=0 ret=0/' \
        "$good" >"$tmp/p.log" || true
    neg "ret=0 (DebugPutStr không in được gì)" "$tmp/p.log"

    # **KT-273** — hai ca âm cho bất biến PCI/blk (chỉ **thêm** kiểm, không bỏ kiểm cũ).
    grep -v -F '# pci: ecam ' "$good" >"$tmp/q.log" || true
    neg "thiếu dòng '# pci: ecam ' (ECAM không chạy)" "$tmp/q.log"

    grep -v -F '# blk: sector0 ' "$good" >"$tmp/r.log" || true
    neg "thiếu dòng '# blk: sector0 ' (không đọc được đĩa)" "$tmp/r.log"

    # **KT-275** — bốn ca âm cho bất biến mạng (chỉ **thêm** kiểm, không bỏ kiểm cũ).
    grep -v -F '# net: device ' "$good" >"$tmp/s.log" || true
    neg "thiếu dòng '# net: device ' (không có card mạng)" "$tmp/s.log"

    sed 's/^# net: dhcp ip=10\.0\.2\.15 gw=10\.0\.2\.2 mask=255\.255\.255\.0 server=10\.0\.2\.2$/# net: dhcp error=Timeout/' \
        "$good" >"$tmp/t.log" || true
    neg "DHCP timeout (không lấy được địa chỉ)" "$tmp/t.log"

    sed 's/^# net: arp gw=52:55:0a:00:02:02$/# net: arp gw=00:00:00:00:00:00/' \
        "$good" >"$tmp/u.log" || true
    neg "ARP trả MAC toàn 0" "$tmp/u.log"

    sed 's/^# net: ping 10\.0\.2\.2 ok n=3 rtt=0$/# net: ping 10.0.2.2 ok n=2 rtt=0/' \
        "$good" >"$tmp/v.log" || true
    neg "ping chỉ 2/3 reply (mất gói)" "$tmp/v.log"
    # **KT-277** — hai ca âm cho bất biến W^X/cache-DMA (chỉ **thêm** kiểm, không bỏ kiểm cũ).
    grep -v -F '# wx: ' "$good" >"$tmp/s.log" || true
    neg "thiếu dòng '# wx: ' (không có bằng chứng W^X)" "$tmp/s.log"

    grep -v -F '# dma: ' "$good" >"$tmp/t.log" || true
    neg "thiếu dòng '# dma: ' (không có cache maintenance trước DMA)" "$tmp/t.log"

    sed 's/^# dma: cache-clean ok=1 lines=64 /# dma: cache-clean ok=1 lines=0 /' \
        "$good" >"$tmp/u.log" || true
    neg "lines=0 (bỏ 'dc civac')" "$tmp/u.log"

    sed 's/^# wx: text=RX data=RW ok=1 /# wx: text=RX data=RW ok=0 /' \
        "$good" >"$tmp/v.log" || true
    neg "ok=0 (ghi vào .text không fault)" "$tmp/v.log"

    # **KT-279** — bốn ca âm cho bất biến GPU (chỉ **thêm** kiểm, không bỏ kiểm cũ).
    grep -v -F '# gpu fb: ' "$good" >"$tmp/w.log" || true
    neg "thiếu dòng '# gpu fb: ' (không có bằng chứng framebuffer)" "$tmp/w.log"

    sed 's/^# gpu fb: pa=0x42000000 walk=0x42000000 identity=1 /# gpu fb: pa=0x42000000 walk=0x42001000 identity=0 /' \
        "$good" >"$tmp/x.log" || true
    neg "identity=0 (framebuffer không map identity)" "$tmp/x.log"

    sed 's/^# gpu console: origin=8,8 cols=79 rows=59 lines=3 chars=29 newlines=3 scrolls=0 dirty=8,8,88,24 flushes=1 commands=2 matched=29\/29 /# gpu console: origin=8,8 cols=79 rows=59 lines=3 chars=29 newlines=3 scrolls=0 dirty=8,8,88,24 flushes=1 commands=58 matched=29\/29 /' \
        "$good" >"$tmp/y.log" || true
    neg "commands=58 (flush từng ký tự thay vì một lô)" "$tmp/y.log"

    sed 's/^# gpu stats: commands=10 notifies=10 errors=0 timeouts=0 id_mismatch=0 rejects=1 fb_clean_lines=19344 /# gpu stats: commands=10 notifies=10 errors=0 timeouts=0 id_mismatch=0 rejects=1 fb_clean_lines=0 /' \
        "$good" >"$tmp/z.log" || true
    neg "fb_clean_lines=0 (bỏ 'dc civac' trên framebuffer)" "$tmp/z.log"

    # **KT-283** — năm ca âm cho bất biến bàn phím (chỉ **thêm** kiểm, không bỏ kiểm cũ).
    grep -v -F '# input: key a ' "$good" >"$tmp/aa.log" || true
    neg "thiếu dòng '# input: key a ' (không nhận được phím)" "$tmp/aa.log"

    sed 's/^# input: key a code=30 ascii=0x61 /# input: key a code=30 ascii=0x62 /' \
        "$good" >"$tmp/ab.log" || true
    neg "ascii=0x62 (sendkey khác ⇒ ký tự khác)" "$tmp/ab.log"

    sed 's/^# input dma: desc=0x400d6000 avail=0x400d6100 used=0x400d6128 buf=0x400d6400 dma=0x400d6000 walk=0x400d6000 identity=1 /# input dma: desc=0x400d6000 avail=0x400d6100 used=0x400d6128 buf=0x400d6400 dma=0x400d6000 walk=0x400d7000 identity=0 /' \
        "$good" >"$tmp/ac.log" || true
    neg "identity=0 (trang DMA không map identity)" "$tmp/ac.log"

    sed 's/ clean_lines=128 clean_bytes=8192 clean_ops=2 / clean_lines=0 clean_bytes=0 clean_ops=0 /' \
        "$good" >"$tmp/ad.log" || true
    neg "clean_lines=0 (bỏ 'dc civac' trên vùng DMA)" "$tmp/ad.log"

    grep -v -F '# input: ready' "$good" >"$tmp/ae.log" || true
    neg "thiếu dòng '# input: ready' (không arm đệm trước khi bơm phím)" "$tmp/ae.log"

    # **KT-289** — hai ca âm cho bất biến TCP/DNS (chỉ **thêm** kiểm, không bỏ kiểm cũ).
    grep -v -F '# net: tcp ' "$good" | grep -v -F '# net tcp data: ' >"$tmp/af.log" || true
    neg "thiếu dòng '# net: tcp' (không mở/nhận được TCP)" "$tmp/af.log"

    grep -v -F '# net: dns ' "$good" >"$tmp/ag.log" || true
    neg "thiếu dòng '# net: dns' (không có bản ghi A)" "$tmp/ag.log"

    # **KT-294** — `attempts=3` = retransmit vô hạn (trần là 2) ⇒ phải ĐỎ.
    sed 's/ attempts=1 const:timeout_ms=3000$/ attempts=3 const:timeout_ms=3000/' \
        "$good" >"$tmp/ah.log" || true
    neg "attempts=3 (retransmit vô hạn)" "$tmp/ah.log"

    # **KT-305** — `blocks_split=0` (unmap block **không** chia nhỏ, tức xoá cả block) ⇒ phải ĐỎ.
    sed 's/ blocks_split=1$/ blocks_split=0/' "$good" >"$tmp/ai.log" || true
    neg "blocks_split=0 (unmap block không chia nhỏ)" "$tmp/ai.log"

    # **KT-408e** (D2-263) — ba ca âm mới cho pha SMP/PAN (TAP 47..50). Chỉ đếm số dòng `ok` là
    # **không đủ**: ảnh in lặp một ca cũ vẫn đủ 50 dòng. Thiếu dòng bằng chứng hoặc sai nhãn TAP 50
    # ⇒ soak phải ĐỎ.
    grep -v -F '# smp core: ' "$good" >"$tmp/aj.log" || true
    neg "thiếu '# smp core:' (mất tổng kết bốn hart tầng core)" "$tmp/aj.log"

    grep -v -E '^# pan: ' "$good" >"$tmp/ak.log" || true
    neg "thiếu '# pan:' (không có bằng chứng PAN)" "$tmp/ak.log"

    sed 's/^ok 50 - pan: el1 khong doc trang el0$/ok 50 - pan: nhan-khac/' \
        "$good" >"$tmp/al.log" || true
    neg "TAP 50 sai nhãn (không phải 'pan: el1 khong doc trang el0')" "$tmp/al.log"

    # ---------------------------------------------------------------- KT-348b — ca kiểm có răng
    # (a) log THIẾU phải báo `không có log` **kèm lý do** (KT-348: 7 boot 814..823 chỉ báo
    #     "không có log" trống, không ai thấy `No space left on device`).
    cases=$((cases + 1))
    if check_boot_log "$tmp/khong-co.log" "$wb" "$wt" \
        "qemu-start: log-open: No space left on device" >"$tmp/miss.out" 2>&1; then
        echo "not ok $cases - ca âm (log thiếu): bộ đọc vẫn XANH"
        fails=$((fails + 1))
    elif grep -q 'không có log' "$tmp/miss.out" &&
        grep -q 'No space left on device' "$tmp/miss.out"; then
        echo "ok $cases - ca âm (log thiếu) ⇒ ĐỎ kèm lý do: $(head -1 "$tmp/miss.out")"
    else
        echo "not ok $cases - ca âm (log thiếu): ĐỎ nhưng KHÔNG nêu lý do ($(head -1 "$tmp/miss.out"))"
        fails=$((fails + 1))
    fi

    # (b) **đếm** tệp log: đủ 3/3 ⇒ XANH; xoá 1 tệp ⇒ ĐỎ + nêu tên tệp + lý do lấy từ results.txt.
    ldir="$tmp/logs"
    mkdir -p "$ldir"
    i=1
    while [ "$i" -le 3 ]; do
        # tệp có nội dung (không rỗng) — ca "rỗng" ở (b2) mới là ngoại lệ duy nhất.
        printf '# boot %s\n' "$i" >"$ldir/boot-$i.log"
        i=$((i + 1))
    done
    printf 'ok 1\nfail 2 start-log-open:_No_space_left_on_device\nok 3\n' >"$ldir/results.txt"
    cases=$((cases + 1))
    if soak_a64_log_inventory 3 "$ldir" >"$tmp/inv-ok.out" 2>&1 &&
        grep -q 'có đủ 3/3 tệp boot-<i>.log' "$tmp/inv-ok.out"; then
        echo "ok $cases - đếm log: đủ 3/3 ⇒ XANH"
    else
        echo "not ok $cases - đếm log: đủ 3/3 mà vẫn ĐỎ ($(head -1 "$tmp/inv-ok.out"))"
        fails=$((fails + 1))
    fi
    rm -f "$ldir/boot-2.log"
    cases=$((cases + 1))
    if soak_a64_log_inventory 3 "$ldir" >"$tmp/inv.log" 2>&1; then
        echo "not ok $cases - đếm log: thiếu 1 tệp mà vẫn XANH (mất bằng chứng im lặng)"
        fails=$((fails + 1))
    elif grep -q 'thiếu 1 tệp, rỗng 0 tệp' "$tmp/inv.log" &&
        grep -q 'boot-2.log (lý do: start-log-open:_No_space_left_on_device)' "$tmp/inv.log"; then
        echo "ok $cases - đếm log: thiếu 1 tệp ⇒ ĐỎ + tên tệp + lý do ($(grep 'không có log' "$tmp/inv.log" | head -1))"
    else
        echo "not ok $cases - đếm log: ĐỎ nhưng thiếu tên/lý do ($(head -2 "$tmp/inv.log" | tr '\n' '|'))"
        fails=$((fails + 1))
    fi

    # (b2) **tệp rỗng** cũng là mất bằng chứng (D2-257: "có mặt + khác rỗng") ⇒ ĐỎ + lý do.
    : >"$ldir/boot-2.log"
    cases=$((cases + 1))
    if soak_a64_log_inventory 3 "$ldir" >"$tmp/inv-empty.log" 2>&1; then
        echo "not ok $cases - đếm log: 1 tệp RỖNG mà vẫn XANH"
        fails=$((fails + 1))
    elif grep -q 'rỗng 1 tệp' "$tmp/inv-empty.log" &&
        grep -q 'log rỗng .*boot-2.log (lý do: ' "$tmp/inv-empty.log"; then
        echo "ok $cases - đếm log: 1 tệp RỖNG ⇒ ĐỎ + tên tệp + lý do ($(grep 'log rỗng' "$tmp/inv-empty.log" | head -1))"
    else
        echo "not ok $cases - đếm log: tệp rỗng mà ĐỎ thiếu tên/lý do ($(head -2 "$tmp/inv-empty.log" | tr '\n' '|'))"
        fails=$((fails + 1))
    fi
    : >"$ldir/boot-2.log"

    # (c) `qg_start` phải **fail-closed có tên** khi không mở được log (ENOSPC/ENOENT):
    #     rc=125 + `QG_START_ERR` chứa errno, thay vì trả 0 rồi caller báo "không có log".
    cases=$((cases + 1))
    if ! command -v perl >/dev/null 2>&1; then
        echo "not ok $cases - ca âm (qg_start không mở được log): thiếu perl (harness cần perl)"
        fails=$((fails + 1))
    else
        # shellcheck source=/dev/null
        . "$HERE/qemu_guard.sh"
        QG_DIR="$tmp"
        if qg_init 2>"$tmp/qginit.err"; then
            rc_log=0
            qg_start "$tmp/khong-co-thu-muc/boot.log" 200 /usr/bin/true 2>/dev/null || rc_log=$?
            if [ "$rc_log" -eq 125 ] && printf '%s' "$QG_START_ERR" | grep -q 'log-open'; then
                echo "ok $cases - ca âm (qg_start log-open) ⇒ rc=125 + QG_START_ERR='$QG_START_ERR'"
            else
                echo "not ok $cases - ca âm (qg_start log-open): rc=$rc_log QG_START_ERR='${QG_START_ERR:-}'"
                fails=$((fails + 1))
            fi
        else
            echo "not ok $cases - ca âm (qg_start log-open): qg_init thất bại ($(head -1 "$tmp/qginit.err" 2>/dev/null))"
            fails=$((fails + 1))
        fi
    fi

    # (d) helper gửi phím thật với **HMP giả** (không QEMU): nối monitor TRƯỚC, chờ dòng ready,
    #     gửi `sendkey a`, rc=0. Ca âm: không có monitor ⇒ rc≠0 + `reason=connect-…`.
    py="$(command -v python3 2>/dev/null || true)"
    kdir="$(v2_env_sockdir kt348b 2>/dev/null || true)"
    cases=$((cases + 1))
    if [ -z "$py" ] || [ -z "$kdir" ]; then
        echo "not ok $cases - helper sendkey: thiếu python3 hoặc SOCKDIR ngắn"
        fails=$((fails + 1))
    else
        : >"$tmp/kready.log"
        : >"$tmp/kbd.log"
        : >"$tmp/kbd-rec.txt"
        "$py" - "$kdir/mon.sock" "$tmp/kbd-rec.txt" <<'PY' &
import socket
import sys

s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.bind(sys.argv[1])
s.listen(1)
c, _ = s.accept()
c.sendall(b"QEMU 11.1.3 monitor - type 'help' for more information\r\n(qemu) ")
buf = b""
while b"\n" not in buf:
    d = c.recv(4096)
    if not d:
        break
    buf += d
open(sys.argv[2], "wb").write(buf)
c.sendall(b"(qemu) ")
c.close()
s.close()
PY
        srv=$!
        (sleep 0.2 && printf '# input: ready\n' >>"$tmp/kready.log") &
        hrc=0
        "$py" "$HERE/lib/monitor-sendkey.py" "$kdir/mon.sock" "$tmp/kready.log" 3000 \
            "$tmp/kbd.log" 7 >"$tmp/kbd.out" 2>&1 || hrc=$?
        if kill -0 "$srv" 2>/dev/null; then kill "$srv" 2>/dev/null; fi
        wait "$srv" 2>/dev/null
        if [ "$hrc" -eq 0 ] && grep -q '^i=7 ok ready_ms=[0-9][0-9]* send_ms=[0-9][0-9]*$' "$tmp/kbd.out" &&
            grep -q 'sendkey a' "$tmp/kbd-rec.txt"; then
            echo "ok $cases - helper sendkey: nối TRƯỚC + gửi khi thấy ready ⇒ rc=0 ($(cat "$tmp/kbd.out"))"
        else
            echo "not ok $cases - helper sendkey dương: rc=$hrc out=$(cat "$tmp/kbd.out" 2>/dev/null) rec=$(cat "$tmp/kbd-rec.txt" 2>/dev/null)"
            fails=$((fails + 1))
        fi
        cases=$((cases + 1))
        hrc=0
        "$py" "$HERE/lib/monitor-sendkey.py" "$kdir/mon-khong-co.sock" "$tmp/kready.log" 500 \
            "$tmp/kbd-neg.log" 8 >"$tmp/kbd-neg.out" 2>&1 || hrc=$?
        if [ "$hrc" -ne 0 ] && grep -q '^i=8 fail reason=connect-' "$tmp/kbd-neg.out"; then
            echo "ok $cases - helper ca âm (không có monitor) ⇒ rc=$hrc + $(cat "$tmp/kbd-neg.out") (không im lặng)"
        else
            echo "not ok $cases - helper ca âm: rc=$hrc out=$(cat "$tmp/kbd-neg.out" 2>/dev/null)"
            fails=$((fails + 1))
        fi
        # (e) monitor **nhận kết nối rồi đóng ngay** (HMP chết, không banner) ⇒ phải ĐỎ có tên,
        #     không treo và không được coi là "đã gửi".
        "$py" - "$kdir/mon-dong.sock" <<'PY' &
import socket
import sys

s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.bind(sys.argv[1])
s.listen(1)
c, _ = s.accept()
c.close()
s.close()
PY
        srv=$!
        cases=$((cases + 1))
        hrc=0
        "$py" "$HERE/lib/monitor-sendkey.py" "$kdir/mon-dong.sock" "$tmp/kready.log" 500 \
            "$tmp/kbd-closed.log" 9 >"$tmp/kbd-closed.out" 2>&1 || hrc=$?
        if kill -0 "$srv" 2>/dev/null; then kill "$srv" 2>/dev/null; fi
        wait "$srv" 2>/dev/null
        if [ "$hrc" -ne 0 ] && grep -q '^i=9 fail reason=banner-eof$' "$tmp/kbd-closed.out"; then
            echo "ok $cases - helper ca âm (monitor đóng ngay) ⇒ rc=$hrc + $(cat "$tmp/kbd-closed.out") (không im lặng)"
        else
            echo "not ok $cases - helper ca âm (monitor đóng): rc=$hrc out=$(cat "$tmp/kbd-closed.out" 2>/dev/null)"
            fails=$((fails + 1))
        fi
        rm -rf "$kdir" 2>/dev/null || true
    fi

    # ---------------------------------------------------------------- KT-408e — SSOT soak==run==kbin
    # (D2-263) Khẳng định `soak.TAP_TESTS == run.EXPECT_TESTS == kbin.TAP_TESTS` cho **cả**
    # aarch64 lẫn x86_64 trên **mã thật** (`lib/tap-parity.sh`), kèm 11 ca (1 dương + 10 âm) trên
    # bản sao tạm: lệch ±1 ở từng nguồn, thiếu tệp, parse hỏng, khai báo trùng, mất call-site.
    # Chạy mọi T2 (0 bước cổng mới) ⇒ lệch TAP bị bắt trong ~0,3 s thay vì 971 s như Peak 2.
    tp_from=$((cases + 1))
    tap_parity_cases "$tp_from"
    cases=$((cases + TAP_PARITY_CASES))
    fails=$((fails + TAP_PARITY_FAILS))

    rm -rf "$tmp" 2>/dev/null || true
    echo "# soak-aarch64 --selftest: $((cases - fails))/$cases ca đúng như phải"
    [ "$fails" -eq 0 ] || return 1
    echo "# soak-aarch64 --selftest: PASS"
    return 0
}

# ------------------------------------------------------------------ đối số
if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    usage
    exit 0
fi
if [ "${1:-}" = "--print-sock" ]; then
    # **KT-333d** — in đường dẫn socket (ngắn) rồi thoát; dùng cho ca âm `$TMPDIR` dài.
    _d="$(v2_env_sockdir kt)" || die "không tạo được SOCKDIR"
    v2_env_sock "$_d" mon-1.sock
    rm -rf "$_d" 2>/dev/null || true
    exit 0
fi
if [ "${1:-}" = "--selftest" ]; then
    SELFTEST=1
    shift
fi
if [ "${1:-}" = "--smoke" ]; then
    SMOKE=1
    shift
    [ $# -ge 1 ] || die "--smoke cần N (ví dụ: --smoke 3 3)"
fi
N="${1:-1000}"
LANES="${2:-3}"
case "$N" in '' | *[!0-9]*) die "N phải là số dương (nhận '$N')" ;; esac
case "$LANES" in '' | *[!0-9]*) die "LANES phải là số dương (nhận '$LANES')" ;; esac
[ "$N" -gt 0 ] || die "N phải > 0"
# AGENTS §9: tối đa 3 QEMU song song; D2-24 đúng 3 luồng cho cổng soak.
[ "$LANES" -ge 1 ] && [ "$LANES" -le 3 ] || die "LANES=$LANES ngoài [1,3] (AGENTS §9: tối đa 3 QEMU)"

# D2-24: n ≥ 3/p. p = 3/1000 ⇒ n ≥ ceil(3 · 1000 / 3) = 1000. Số nguyên, không dấu phẩy động.
NEED=$(((3 * P_DEN + P_NUM - 1) / P_NUM))
if [ "$N" -lt "$NEED" ]; then
    if [ "$SMOKE" -ne 1 ]; then
        echo "soak-aarch64.sh: N=$N < n ≥ 3/p = $NEED (p=$P_NUM/$P_DEN = 0,3 %; D2-24, QD §5.3)" >&2
        echo "soak-aarch64.sh: giảm N là NỚI NGƯỠNG — không chạy. Dùng --smoke chỉ để đỏ trước/kiểm marker." >&2
        exit 2
    fi
    echo "# soak: SMOKE N=$N < $NEED — KHÔNG phải bằng chứng cổng K4' (D2-24), luôn exit 1"
fi

# ------------------------------------------------------------------ --selftest (không cần tool/QEMU)
if [ "$SELFTEST" -eq 1 ]; then
    selftest
    exit $?
fi

# ------------------------------------------------------------------ công cụ
command -v cargo >/dev/null 2>&1 || die "không có cargo"
command -v python3.12 >/dev/null 2>&1 || die "không có python3.12 (cần cho image_digest.py)"
# **KT-283**: `python3` để gửi `sendkey a` qua HMP monitor (cùng yêu cầu `run-aarch64.sh`).
PYTHON="$(command -v python3 || true)"
[ -n "$PYTHON" ] || die "không có python3 (cần cho monitor sendkey)"
[ -f "$DIGEST" ] || die "không thấy $DIGEST"
[ -f "$BUDGET" ] || die "không thấy $BUDGET"
QEMU="$(command -v qemu-system-aarch64 || true)"
[ -n "$QEMU" ] || die "không có qemu-system-aarch64"

# QEMU >= 9.2 (cùng sàn riscv64, D2-72 Q2) — thiếu/dưới sàn ⇒ đỏ, không tự hạ.
QEMU_VERSION="$("$QEMU" --version 2>/dev/null | sed -n '1s/.*version \([0-9][^ ]*\).*/\1/p')"
[ -n "$QEMU_VERSION" ] || die "không đọc được phiên bản QEMU"
case "$QEMU_VERSION" in *.*) ;; *) die "phiên bản QEMU '$QEMU_VERSION' không có dạng major.minor" ;; esac
QEMU_MAJOR="${QEMU_VERSION%%.*}"
QEMU_REST="${QEMU_VERSION#*.}"
QEMU_MINOR="${QEMU_REST%%.*}"
case "$QEMU_MAJOR" in '' | *[!0-9]*) die "không đọc được major của QEMU '$QEMU_VERSION'" ;; esac
case "$QEMU_MINOR" in '' | *[!0-9]*) die "không đọc được minor của QEMU '$QEMU_VERSION'" ;; esac
if [ "$QEMU_MAJOR" -lt "$QEMU_MIN_MAJOR" ] ||
    { [ "$QEMU_MAJOR" -eq "$QEMU_MIN_MAJOR" ] && [ "$QEMU_MINOR" -lt "$QEMU_MIN_MINOR" ]; }; then
    die "QEMU $QEMU_VERSION < sàn $QEMU_MIN_MAJOR.$QEMU_MIN_MINOR (D2-72 Q2) — không tự hạ"
fi

# Cấu hình QEMU **đúng** đường boot của ảnh (k4-aarch64-boot.md §2 + run-aarch64.sh):
# `gic-version=3` bắt buộc từ KT-259 (GICv2 không có redistributor/`ICC_*_EL1`), `-smp 4` từ
# KT-262, `-cpu cortex-a72` như runner. KHÔNG `-icount` (D2-74 điều kiện c).
# **KT-273**: thêm **đĩa tạm** có nội dung biết trước + `-nic none` (topology PCI tất định) —
# cùng cấu hình `run-aarch64.sh`, nếu không ảnh sẽ in `not ok 20/21` và soak đỏ vì lý do sai.
# `snapshot=on` **bắt buộc**: 3 luồng soak dùng **cùng** tệp đĩa, thiếu nó QEMU báo
# `Failed to get "write" lock` và 2/3 boot đỏ (đo được khi dựng KT-273).
# **KT-275**: thêm **card mạng** slirp (`-netdev user,id=n0 -device virtio-net-pci,netdev=n0`) —
# DHCP/ARP/ICMP cần server thật của slirp; `-nic none` vẫn giữ để **không** có NIC mặc định thứ
# hai (topology PCI tất định: host bridge + virtio-net + virtio-blk). Ba luồng song song mỗi luồng
# một QEMU ⇒ mỗi QEMU một slirp riêng, cùng dải `10.0.2.x`, không tranh cổng host (không hostfwd).
DISK="$LOGDIR/disk0.raw"
# **KT-283**: thêm **bàn phím** (`-device virtio-keyboard-pci`) — ảnh in TAP 31/32 cho
# virtio-input, nên thiếu thiết bị là soak ĐỎ vì lý do sai. `-monitor` **không** nằm trong mảng
# này: mỗi boot mở socket riêng (`boot_keys`) để 1.000 boot không tranh một đường dẫn.
QEMU_ARGS=(-M virt,gic-version=3 -cpu cortex-a72 -m 128M -nographic -smp 4 \
    -kernel "$IMAGE" -no-reboot -nic none \
    -netdev user,id=n0 -device virtio-net-pci,netdev=n0 \
    -drive "file=$DISK,if=none,id=d0,format=raw,snapshot=on" -device virtio-blk-pci,drive=d0 \
    -device virtio-gpu-pci -device virtio-keyboard-pci)

# ------------------------------------------------------------------ build + neo ảnh
echo "# soak-aarch64 KT-265 — $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
echo "# qemu $QEMU_VERSION (sàn $QEMU_MIN_MAJOR.$QEMU_MIN_MINOR, D2-72 Q2)"
echo "# cấu hình: -M virt,gic-version=3 -cpu cortex-a72 -m 128M -nographic -smp 4 -no-reboot; KHÔNG icount (D2-74 c)"
echo "# N=$N lanes=$LANES timeout=$((TIMEOUT_DS / 10))s/boot; p=$P_NUM/$P_DEN (0,3 %) ⇒ n ≥ 3/p = $NEED (D2-24)"
if command -v git >/dev/null 2>&1; then
    GIT_HEAD="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || echo unknown)"
    GIT_DIRTY="$(git -C "$REPO_ROOT" status --porcelain -- v2/kernel 2>/dev/null | wc -l | tr -d ' ')"
    echo "# git HEAD=$GIT_HEAD dirty-kernel-paths=$GIT_DIRTY"
else
    GIT_HEAD=unknown
    GIT_DIRTY=unknown
fi

mkdir -p "$LOGDIR" 2>/dev/null || true
# **KT-273** — đĩa tạm: 8 sector 512 B, 17 byte đầu `VIFASTOS-KT273-S0` (8 byte đầu khớp
# `virtio_blk::SECTOR0_MAGIC`). Soak đòi **cùng** hex 16 byte đầu trong mỗi log.
dd if=/dev/zero of="$DISK" bs=512 count=8 2>/dev/null || die "không tạo được đĩa tạm $DISK"
printf 'VIFASTOS-KT273-S0' | dd of="$DISK" bs=1 seek=0 conv=notrunc 2>/dev/null ||
    die "không ghi được chữ ký vào $DISK"
WANT_SECTOR0_HEX="$(od -An -tx1 -N16 "$DISK" 2>/dev/null | tr -d ' \n')"
[ "${#WANT_SECTOR0_HEX}" -eq 32 ] || die "không đọc được 16 byte đầu của $DISK"
# KT-78/OD-K6-6: boot đi qua `qemu_guard.sh` — process group riêng, timeout kill **cả group**,
# trap dọn sạch, watchdog dọn khi cha chết (kể cả SIGKILL). 3 luồng = 3 subshell cùng script.
# shellcheck source=/dev/null
. "$HERE/qemu_guard.sh"
QG_DIR="${TMPDIR:-/tmp}"
QG_LOG="$LOGDIR/qemu-guard.log"
rm -f "$QG_LOG" 2>/dev/null # log guard **mới** mỗi lần chạy (tổng hợp theo run)
qg_init || die "qemu_guard: không khởi tạo được (thiếu perl?) — OD-K6-6"
INFRA_INVALID=0
INFRA_LOG="$LOGDIR/kt265-infra.log"
qg_infra before "$INFRA_LOG" || INFRA_INVALID=$((INFRA_INVALID + 1))
# **KT-333c** (D2-243 mục 3a) — khoá **độc quyền cấp máy đo**, giữ từ trước build tới hết soak
# (2 soak song song vừa tranh `kernel/target` vừa chồng QEMU). Nhả chỉ ở shell chính.
# shellcheck source=/dev/null
. "$V2_ROOT/tools/measure-lock.sh"
measure_lock_acquire "soak aarch64 N=$N lanes=$LANES" || exit 2
# **KT-348b (D2-257 mục 1)** — **một** `trap EXIT` duy nhất. Bản cũ đặt trap ở đây, rồi đặt lại
# ở chỗ TCP/DNS server và **đặt lại lần nữa** sau `SOCKDIR` ⇒ hai trap sau **ghi đè** trap trước;
# `stop_soak_dns_server`/`stop_soak_tcp_server`/`qg_cleanup` **không bao giờ chạy** khi script thoát
# (đường `die`/`exit 3`: server + QEMU group rò rỉ). Gộp **cả bốn** việc dọn vào một hàm.
# Chết trước khi phần TCP/DNS định nghĩa hàm ⇒ `type -t` bỏ qua an toàn (không "command not found").
_soak_a64_cleanup() {
    if type -t stop_soak_dns_server >/dev/null 2>&1; then stop_soak_dns_server || true; fi
    if type -t stop_soak_tcp_server >/dev/null 2>&1; then stop_soak_tcp_server || true; fi
    qg_cleanup 2>/dev/null || true
    [ -n "${SOCKDIR:-}" ] && rm -rf "$SOCKDIR" 2>/dev/null
    measure_lock_release_if_top 2>/dev/null || true
}
# **KT-407** — một lời gọi duy nhất; `run_common_trap` cài đúng một bộ EXIT/INT/TERM/HUP.
run_common_trap _soak_a64_cleanup

# ------------------------------------------------------------------ TCP server trên host (KT-289)
# Ảnh aarch64 in TAP 33/34 cho **TCP client**; khách nối `10.0.2.2:45678` → slirp ánh xạ về
# `127.0.0.1:45678` của host (`sotranslate_out4`; `hostfwd` là chiều host → guest nên **không**
# dùng được — KT-268). Soak chạy **3 luồng song song** ⇒ **một** server nhận **nhiều** kết nối
# (mỗi boot một kết nối), xử lý tuần tự; log ghi từng mốc để đếm bằng chứng hai chiều. Cùng khuôn
# `net.sh`/`run-aarch64.sh` (KT-268/KT-289) — không có bản thứ hai lệch hành vi.
TCP_PORT=45678
TCP_TOKEN="vifast-tcp-ok"
TCP_TOKEN_LEN=${#TCP_TOKEN}
TCP_SEND_LEN=15 # `vifast-tcp-send`
TCP_SERVER_PID=""
TCP_SERVER_LOG=""

write_soak_tcp_server() {
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
srv.listen(8)
note("listen port=%d" % port)
srv.settimeout(1.0)
while True:
    try:
        conn, addr = srv.accept()
    except socket.timeout:
        continue
    except OSError:
        break
    note("accept peer=%s:%d" % addr)
    try:
        conn.sendall(token)
        note("sent=%d" % len(token))
        conn.settimeout(5.0)
        total = 0
        while True:
            data = conn.recv(4096)
            if not data:
                break
            total += len(data)
        note("recv=%d" % total)
    except OSError as error:
        note("conn_err=%s" % error)
    finally:
        conn.close()
PY
}

start_soak_tcp_server() {
    local i
    command -v python3 >/dev/null 2>&1 || die "không có python3 để dựng TCP server (KT-289)"
    write_soak_tcp_server
    TCP_SERVER_LOG="$LOGDIR/kt289-tcp-server.log"
    rm -f "$TCP_SERVER_LOG"
    python3 "$LOGDIR/kt289-tcp-server.py" "$TCP_PORT" "$TCP_TOKEN" "$TCP_SERVER_LOG" &
    TCP_SERVER_PID=$!
    for i in $(seq 1 50); do
        grep -q '^listen ' "$TCP_SERVER_LOG" 2>/dev/null && return 0
        if grep -q '^bind_err=' "$TCP_SERVER_LOG" 2>/dev/null; then
            cat "$TCP_SERVER_LOG" >&2
            die "TCP server không bind được 127.0.0.1:$TCP_PORT — port đang bận?"
        fi
        sleep 0.1
    done
    die "TCP server không lắng nghe sau 5 s"
}

stop_soak_tcp_server() {
    if [ -n "$TCP_SERVER_PID" ]; then
        kill "$TCP_SERVER_PID" 2>/dev/null || true
        wait "$TCP_SERVER_PID" 2>/dev/null || true
        TCP_SERVER_PID=""
    fi
}

# ------------------------------------------------------------------ DNS server cục bộ (KT-300)
# Vì sao phải có (khảo sát đầy đủ ở header): QEMU/slirp **luôn** chuyển DNS tới resolver của
# host; `dns=` chỉ đổi địa chỉ DNS ảo, `guestfwd` chỉ TCP, `hostfwd` ngược chiều. Muốn DNS cục
# bộ **chỉ trong soak** mà không đổi ảnh/T2: interpose `res_getservers` của **tiến trình QEMU**
# (dylib) trỏ resolver thượng nguồn về `127.0.0.1:53`, nơi server python3 dưới đây trả A cố định.
# `0.0.0.0:53` (không phải `127.0.0.1:53`): macOS cho bind **không cần root** ở địa chỉ wildcard,
# còn `127.0.0.1:53` trả EACCES (đo được). Server chỉ trả lời `example.com`; tên khác NXDOMAIN.
DNS_PORT=53
DNS_ANSWER_IP="192.0.2.123" # TEST-NET-1: resolver thật **không** bao giờ trả địa chỉ này
DNS_TTL=3600
DNS_MUTATION="${SOAK_DNS_MUTATION:-}" # "" | off | nxdomain | wrongip — chỉ dùng cho đột biến
DNS_SERVER_PID=""
DNS_SERVER_LOG=""
DNS_DYLIB=""
# **KT-406** — nhánh nạp thư viện theo OS: `DYLD_INSERT_LIBRARIES` (macOS) hoặc `LD_PRELOAD`
# (Linux). Tên biến + loại (.dylib/.so) do `dns-interpose.sh` trả về, không đoán theo `uname` ở đây.
DNS_PRELOAD_VAR=""
DNS_KIND=""
DNS_WRAPPER=""
DNS_REAL_QEMU=""

write_soak_dns_server() {
    cat >"$LOGDIR/kt300-dns-server.py" <<'PY'
import socket, struct, sys

port = int(sys.argv[1])
log = sys.argv[2]
mode = sys.argv[3] if len(sys.argv) > 3 else "ok"
answer_ip = sys.argv[4] if len(sys.argv) > 4 else "192.0.2.123"
ttl = int(sys.argv[5]) if len(sys.argv) > 5 else 3600


def note(line):
    with open(log, "a") as handle:
        handle.write(line + "\n")


def parse_qname(data, off):
    labels = []
    while True:
        if off >= len(data):
            return None, off
        ln = data[off]
        off += 1
        if ln == 0:
            break
        if ln & 0xC0:
            return None, off
        labels.append(data[off:off + ln].decode("latin-1"))
        off += ln
    return ".".join(labels), off


srv = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
try:
    srv.bind(("0.0.0.0", port))
except OSError as error:
    note("bind_err=%s" % error)
    sys.exit(2)
note("listen port=%d mode=%s answer=%s ttl=%d" % (port, mode, answer_ip, ttl))
srv.settimeout(1.0)
while True:
    try:
        data, addr = srv.recvfrom(2048)
    except socket.timeout:
        continue
    except OSError:
        break
    if len(data) < 12:
        continue
    tid, flags, qd, an, ns, ar = struct.unpack("!HHHHHH", data[:12])
    qname, off = parse_qname(data, 12)
    if qname is None or off + 4 > len(data):
        note("bad_query len=%d" % len(data))
        continue
    qtype, qclass = struct.unpack("!HH", data[off:off + 4])
    qend = off + 4
    note("query id=0x%04x name=%s type=%d class=%d from=%s:%d"
         % (tid, qname, qtype, qclass, addr[0], addr[1]))
    if qname != "example.com" or qtype != 1:
        rcode = 3
        answer = b""
        ancount = 0
    elif mode == "nxdomain":
        rcode = 3
        answer = b""
        ancount = 0
    elif mode == "wrongip":
        rcode = 0
        answer = b"\xc0\x0c" + struct.pack("!HHIH", 1, 1, ttl, 4) + socket.inet_aton("203.0.113.9")
        ancount = 1
    else:
        rcode = 0
        answer = b"\xc0\x0c" + struct.pack("!HHIH", 1, 1, ttl, 4) + socket.inet_aton(answer_ip)
        ancount = 1
    resp = struct.pack("!HHHHHH", tid, 0x8180 | rcode, 1, ancount, 0, 0) + data[12:qend] + answer
    srv.sendto(resp, addr)
    note("reply id=0x%04x rcode=%d an=%d len=%d" % (tid, rcode, ancount, len(resp)))
PY
}

start_soak_dns_server() {
    local i
    if [ "$DNS_MUTATION" = "off" ]; then
        echo "# dns server: KHÔNG dựng (SOAK_DNS_MUTATION=off — đột biến KT-300)"
        return 0
    fi
    command -v python3 >/dev/null 2>&1 || die "không có python3 để dựng DNS server (KT-300)"
    write_soak_dns_server
    DNS_SERVER_LOG="$LOGDIR/kt300-dns-server.log"
    rm -f "$DNS_SERVER_LOG"
    python3 "$LOGDIR/kt300-dns-server.py" "$DNS_PORT" "$DNS_SERVER_LOG" "$DNS_MUTATION" \
        "$DNS_ANSWER_IP" "$DNS_TTL" &
    DNS_SERVER_PID=$!
    for i in $(seq 1 50); do
        grep -q '^listen ' "$DNS_SERVER_LOG" 2>/dev/null && return 0
        if grep -q '^bind_err=' "$DNS_SERVER_LOG" 2>/dev/null; then
            cat "$DNS_SERVER_LOG" >&2
            die "DNS server không bind được 0.0.0.0:$DNS_PORT — port đang bận? (KT-300)"
        fi
        sleep 0.1
    done
    die "DNS server không lắng nghe sau 5 s (KT-300)"
}

stop_soak_dns_server() {
    if [ -n "$DNS_SERVER_PID" ]; then
        kill "$DNS_SERVER_PID" 2>/dev/null || true
        wait "$DNS_SERVER_PID" 2>/dev/null || true
        DNS_SERVER_PID=""
    fi
}

# Dylib interpose: thay `res_9_ninit`/`res_9_getservers`/`res_9_ndestroy` (macOS) để
# `get_dns_addr()` của libslirp trả `127.0.0.1:53` — **chỉ** tiến trình QEMU của soak nạp nó.
# **KT-406** — C nguồn + build **một** nguồn ở `v2/test/boot/dns-interpose.sh`: macOS dựng
# `.dylib` (`DYLD_INTERPOSE` trên `res_9_*`), Linux dựng `.so` (`LD_PRELOAD` bọc
# `res_init`/`res_ninit`). Ở đây chỉ **gọi** nó và đọc `lib=`/`kind=`/`var=` — không có bản sao C
# thứ hai để lệch nhau giữa soak và tự kiểm.
write_soak_dns_interpose() {
    : # C nguồn nằm ở `dns-interpose.sh` (một nguồn, hai nhánh OS)
}

build_soak_dns_interpose() {
    local out
    out="$(bash "$HERE/dns-interpose.sh" --lib "$LOGDIR")" ||
        die "biên dịch interpose DNS FAIL (KT-300/KT-406)"
    DNS_DYLIB="$(printf '%s\n' "$out" | sed -n 's/^lib=//p')"
    DNS_KIND="$(printf '%s\n' "$out" | sed -n 's/^kind=//p')"
    DNS_PRELOAD_VAR="$(printf '%s\n' "$out" | sed -n 's/^var=//p')"
    [ -n "$DNS_DYLIB" ] && [ -f "$DNS_DYLIB" ] || die "không thấy thư viện interpose DNS (KT-406)"
    [ -n "$DNS_PRELOAD_VAR" ] || die "dns-interpose.sh không trả biến nạp (KT-406)"
}

# Wrapper QEMU: `qemu_guard.sh` chạy QEMU qua `perl` (binary SIP) nên dyld **xoá** `DYLD_*` khỏi
# môi trường trước `exec`; wrapper (bash thường) đặt lại biến rồi `exec` QEMU thật.
write_soak_qemu_wrapper() {
    DNS_REAL_QEMU="$QEMU"
    DNS_WRAPPER="$LOGDIR/kt300-qemu-dns-wrap.sh"
    cat >"$DNS_WRAPPER" <<SH
#!/bin/bash
# KT-300/KT-406 — chỉ soak: đặt lại biến nạp (DYLD_INSERT_LIBRARIES trên macOS, LD_PRELOAD trên
# Linux) rồi exec QEMU thật.
export "$DNS_PRELOAD_VAR=$DNS_DYLIB"
exec "$DNS_REAL_QEMU" "\$@"
SH
    chmod +x "$DNS_WRAPPER" || die "không chmod được $DNS_WRAPPER (KT-300)"
}

# ---- **KT-338c** (D2-08): ảnh **sẢN PHẨM** (không feature) trước — 0 harness marker + 64 KiB.
echo "# build ảnh SẢN PHẨM aarch64 ($TARGET, release, KHÔNG feature)"
if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-aarch64 --target "$TARGET" --release \
    >"$LOGDIR/kt338c-product-build.log" 2>&1; then
    tail -20 "$LOGDIR/kt338c-product-build.log" >&2
    die "build ảnh sản phẩm aarch64 FAIL (xem $LOGDIR/kt338c-product-build.log)"
fi
[ -f "$IMAGE" ] || die "không thấy ảnh sản phẩm $IMAGE"
PRODUCT_IMAGE="$LOGDIR/vifast-kbin-aarch64-product"
cp "$IMAGE" "$PRODUCT_IMAGE" || die "không copy được ảnh sản phẩm"
# D2-236/D2-244: `.data+.bss` của ảnh **sản phẩm** phải `< 64 KiB` (không nới trần ktest).
bash "$BUDGET" "$PRODUCT_IMAGE" aarch64-product --product ||
    die "budget ảnh SẢN PHẨM vượt trần 64 KiB (D2-236/D2-244)"
# 0 harness marker trong ảnh sản phẩm (D2-08) — **một** nguồn `lib/harness-scan.sh` (23 tiền tố,
# cùng cách đếm dòng `strings` như trước; tệp vắng ⇒ rc 1 chứ không "0 hit" giả).
HITS="$(harness_scan_count_strings "$PRODUCT_IMAGE")" || die "không đọc được ảnh $PRODUCT_IMAGE (TOCTOU)"
[ "${HITS:-0}" -eq 0 ] || die "ảnh sản phẩm chứa ${HITS} harness marker (D2-08)"

echo "# build ảnh KTEST aarch64 ($TARGET, release, --features ktest)"
if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-aarch64 --target "$TARGET" --release \
    --features ktest >"$LOGDIR/kt265-soak-build.log" 2>&1; then
    tail -20 "$LOGDIR/kt265-soak-build.log" >&2
    die "build ảnh ktest aarch64 FAIL (xem $LOGDIR/kt265-soak-build.log)"
fi
[ -f "$IMAGE" ] || die "không thấy ảnh $IMAGE"

# D2-72 Q5: `.data+.sdata+.bss+.sbss` ≤ 262.144 B — fail nếu vượt, không nới.
bash "$BUDGET" "$IMAGE" aarch64-ktest || die "budget ảnh aarch64 VƯỢT trần (D2-72 Q5)"
echo "# image=product ktest-strings=0"

# D2-80: BUILD_ID (FNV-1a 64 trên `.text`, kernel tự in) + sha256 (toàn ảnh) + kích thước.
DIGEST_OUT="$(python3.12 "$DIGEST" "$IMAGE" 2>/dev/null)" || die "image_digest.py lỗi trên $IMAGE"
WANT_BUILD="$(printf '%s\n' "$DIGEST_OUT" | sed -n 's/^build=\([0-9a-f]*\).*/\1/p')"
WANT_TEXT="$(printf '%s\n' "$DIGEST_OUT" | sed -n 's/.* text=\([0-9]*\)$/\1/p')"
[ -n "$WANT_BUILD" ] && [ -n "$WANT_TEXT" ] || die "không đọc được BUILD_ID từ $DIGEST_OUT"
if command -v shasum >/dev/null 2>&1; then
    IMG_SHA="$(shasum -a 256 "$IMAGE" | awk '{print $1}')"
else
    IMG_SHA="$(sha256sum "$IMAGE" | awk '{print $1}')"
fi
[ -n "$IMG_SHA" ] || die "không tính được sha256 ảnh"
IMG_BYTES="$(wc -c <"$IMAGE" | tr -d ' ')"
echo "# image=product bytes=$IMG_BYTES sha256=$IMG_SHA"
echo "# BUILD_ID (D2-80) = build=$WANT_BUILD text=$WANT_TEXT"
if [ "$GIT_DIRTY" != "0" ] && [ "$GIT_DIRTY" != "unknown" ]; then
    PATCH_SHA="$(
        git -C "$REPO_ROOT" diff -- v2/kernel 2>/dev/null | shasum -a 256 | awk '{print $1}'
    )"
    echo "# git dirty: v2/kernel có $GIT_DIRTY tệp khác HEAD; sha256 patch=$PATCH_SHA"
fi

# --------------------------------------------------- cổng danh tính ảnh (D2-80)
# Ảnh aarch64 **chưa** in `# build:` của KT-86 (nợ ở k4-aarch64-boot.md §7), nên cổng danh tính
# ở đây là **một** boot dò: log phải có **đúng một** `# conf image:` khớp ELF. Không có/lệch ⇒
# TỪ CHỐI ĐO (exit 3), **không** đếm một boot soak nào — thà không đo còn hơn đo ảnh không rõ nguồn.
PROBE_LOG="$LOGDIR/kt265-probe.log"
# **KT-300** — dựng interpose + wrapper DNS **trước** mọi boot (probe cũng phải dùng DNS cục bộ).
build_soak_dns_interpose
write_soak_qemu_wrapper
# Trap **chuỗi**: giữ `qg_cleanup` của `qemu_guard.sh` (KT-78) và thêm dọn server TCP + DNS —
# đặt **trước** khi dựng server để `die` giữa chừng cũng không để tiến trình nền lại.
# **KT-348b** — trap EXIT/INT/TERM đã gộp ở `_soak_a64_cleanup` (một chỗ duy nhất).
# **KT-289**: server TCP phải lắng nghe **trước** boot dò (ảnh chạy tới TAP 33/34 ở mọi boot).
start_soak_tcp_server
# **KT-300**: server DNS cục bộ phải lắng nghe **trước** boot dò (ảnh chạy tới TAP 35 ở mọi
# boot), và QEMU từ đây chạy qua wrapper nạp interpose (chỉ soak; `run-aarch64.sh` giữ nguyên).
start_soak_dns_server
QEMU="$DNS_WRAPPER"
echo "# dns server: local=0.0.0.0:$DNS_PORT answer=$DNS_ANSWER_IP ttl=$DNS_TTL mutation=${DNS_MUTATION:-none} dylib=$DNS_DYLIB kind=$DNS_KIND var=$DNS_PRELOAD_VAR"
# **KT-408e** (D2-263) — boot dò **chuyển xuống** ngay sau `boot_keys()` (bên dưới): từ nay nó
# được chấm bằng **chính** `check_boot_log` của soak, nên nó phải là một boot sạch **đầy đủ**
# (monitor + `sendkey a` — ảnh aarch64 có TAP 32 `input: key` phụ thuộc phím). Xem khối
# "cổng danh tính + bất biến" sau `boot_keys()`.

# ------------------------------------------------------------------ log
RUNDIR="$LOGDIR/run-$(date +%Y%m%d-%H%M%S)-$$"
# **KT-295** — socket monitor phải nằm ở đường dẫn **ngắn**: `AF_UNIX` trên macOS giới hạn 104
# byte kể cả NUL, mà `$RUNDIR/mon-<i>.sock` đã dài 104 khi `i >= 100` ⇒ `connect` thất bại.
# Thư mục riêng dưới `/tmp` (ngắn, cùng ổ đĩa) — **không** đổi bất biến/ngưỡng nào.
# **KT-333d (D2-248 mục 2b)** — `env.sh` chọn thư mục socket ngắn theo **một** chính sách chung
# (không còn `mktemp -d /tmp/kt-sock.XXXXXX` rải rác ở từng runner); vẫn kiểm tên dài nhất.
SOCKDIR="$(v2_env_sockdir kt)" || die "không tạo được SOCKDIR"
v2_env_sock "$SOCKDIR" mon-999999.sock >/dev/null ||
    die "đường dẫn socket quá dài (AF_UNIX < 104 byte)"
# **KT-348b** — `SOCKDIR` đã nằm trong `_soak_a64_cleanup` (một trap duy nhất).
mkdir -p "$RUNDIR" || die "không tạo được $RUNDIR"
RESULTS="$RUNDIR/results.txt"
: >"$RESULTS" || die "không ghi được $RESULTS"
echo "# log: $RUNDIR/boot-<i>.log (mỗi boot một tệp; kết quả: $RESULTS)"

# ------------------------------------------------------------------ bơm phím (KT-283/KT-348b)
# **KT-348b — sửa THỨ TỰ gửi (đo được ở KT-348)**: đường cũ chờ `# input: ready` bằng `grep`
# rồi **mới** khởi động `python3` để nối HMP và gửi `sendkey a`. Toàn bộ đường đó nằm trong cửa
# sổ 5 s của ảnh (`input::INPUT_WAIT_SECS`): dưới tải máy đo, các boot có `elapsed_ms`
# 1.578–3.827 và boot 861 chạm **5.076 s** ⇒ ảnh hết hạn, QEMU thoát, lần `connect` sau đó nhận
# `[Errno 2] No such file or directory` (socket monitor đã bị QEMU xoá khi thoát) — harness cũ
# `send_key … || true` **nuốt** lỗi đó nên boot đỏ trông như lỗi nhân `TAP 32 input: key`.
#
# Nay `lib/monitor-sendkey.py` nối monitor **trước** (Python khởi động chạy song song với lúc
# khách boot), poll tệp log bằng `read(2)` (không spawn `grep` mỗi 50 ms) và gửi `sendkey a` trên
# kết nối **đã mở sẵn** ngay khi thấy `# input: ready` (dòng này in **sau** khi ảnh đã arm 8 đệm
# ⇒ gửi an toàn, không sớm hơn điều kiện của ảnh). Mọi lỗi hạ tầng (không nối được monitor,
# monitor chết, không thấy ready, monitor báo `Error`) trả **tên**; boot bị xếp `monitor-…`/
# `ready-…` và **không** đội lốt lỗi nhân. Không nới ngưỡng nào: `READY_DS` = 5 s và cửa sổ 5 s
# của ảnh giữ nguyên.
KEY_HELPER="$HERE/lib/monitor-sendkey.py"
KBD_LOG="$RUNDIR/kbd.log"
: >"$KBD_LOG" || die "không ghi được $KBD_LOG"
INFRA_STOP="$RUNDIR/INFRA-STOP"
INFRA_FAIL="$RUNDIR/INFRA-FAIL.txt"
: >"$INFRA_FAIL" || die "không ghi được $INFRA_FAIL"

# `soak_reason <chuỗi>`: một dòng `results.txt` chỉ có các trường cách nhau bởi dấu cách ⇒ đổi
# khoảng trắng thành `_` (lý do của harness có thể chứa ':', '/', errno… nhưng không được chứa
# khoảng trắng, nếu không phần đọc lại sẽ tách sai).
soak_reason() {
    printf '%s' "$1" | tr '\n\t ' ';;_' | sed 's/__*/_/g'
}

# `boot_keys <log> <sock> <i>`: như `qg_boot` nhưng giữ **HMP monitor** và bơm `sendkey a`.
#
# Trả 0 = QEMU tự thoát sau khi phím đã gửi; 124 = quá hạn (đã kill cả group);
# 125 = **không khởi động được** (QEMU / không mở được log) — `QG_START_ERR` có lý do;
# 126 = **harness không gửi được phím** — `QG_KEYS_ERR` có lý do (T-INFRA, không phải lỗi nhân).
boot_keys() {
    local out="$1" sock="$2" i="$3"
    local pid waited t0 tmax kpid krc budget tmax2 g=0
    rm -f "$sock" 2>/dev/null
    qg_tmp "$sock"
    QG_KEYS_ERR=""
    qg_start "$out" "$TIMEOUT_DS" "$QEMU" "${QEMU_ARGS[@]}" \
        -monitor "unix:$sock,server=on,wait=off" || return 125
    pid="$QG_LAST_PID"
    waited="${QG_START_SPENT:-0}"
    # Helper nối monitor **ngay** — trước khi khách in ready — rồi poll log và gửi.
    "$PYTHON" "$KEY_HELPER" "$sock" "$out" "$((READY_DS * 50))" "$KBD_LOG" "$i" \
        >"$RUNDIR/kbd-$i.txt" 2>&1 &
    kpid=$!
    t0=$SECONDS
    tmax=$(((TIMEOUT_DS / 10) * 2 + 5))
    # Chờ **kết quả đã ghi** (không dựa `kill -0`: tiến trình con đã thoát vẫn là zombie ⇒
    # `kill -0` vẫn trả 0) hoặc QEMU chết hoặc hết ngân sách.
    while [ ! -s "$RUNDIR/kbd-$i.txt" ] && kill -0 "$pid" 2>/dev/null &&
        [ "$waited" -lt "$TIMEOUT_DS" ] && [ $((SECONDS - t0)) -lt "$tmax" ]; do
        sleep 0.1
        waited=$((waited + 1))
    done
    # QEMU có thể chết trước khi helper kịp ghi (nó tự phát hiện EOF) — cho tối đa 0,5 s để ghi.
    if [ ! -s "$RUNDIR/kbd-$i.txt" ] && ! kill -0 "$pid" 2>/dev/null; then
        while [ "$g" -lt 10 ] && [ ! -s "$RUNDIR/kbd-$i.txt" ]; do
            sleep 0.05
            g=$((g + 1))
        done
    fi
    if [ ! -s "$RUNDIR/kbd-$i.txt" ] && kill -0 "$kpid" 2>/dev/null; then
        kill "$kpid" 2>/dev/null
    fi
    wait "$kpid" 2>/dev/null
    krc=$?
    if [ "$krc" -ne 0 ] || ! grep -q '^i=[0-9][0-9]* ok ' "$RUNDIR/kbd-$i.txt" 2>/dev/null; then
        QG_KEYS_ERR="$(sed -n 's/.* reason=\([^ ]*\).*/\1/p' "$RUNDIR/kbd-$i.txt" 2>/dev/null | head -1)"
        [ -n "$QG_KEYS_ERR" ] || QG_KEYS_ERR="no-result-rc$krc"
        if kill -0 "$pid" 2>/dev/null; then
            qg_kill_group "$pid"
        fi
        wait "$pid" 2>/dev/null
        QG_LAST_RC=126
        qg_guard_note boot_end
        return 126
    fi
    # Đã gửi phím: chờ QEMU tự thoát trong phần ngân sách **còn lại** (cùng công thức `qg_boot`).
    budget=$((TIMEOUT_DS - waited))
    [ "$budget" -lt 10 ] && budget=10
    t0=$SECONDS
    tmax2=$(((budget / 10) * 2 + 5))
    while kill -0 "$pid" 2>/dev/null && [ "$waited" -lt "$TIMEOUT_DS" ] &&
        [ $((SECONDS - t0)) -lt "$tmax2" ]; do
        sleep 0.1
        waited=$((waited + 1))
    done
    QG_LAST_SECS="$waited"
    if kill -0 "$pid" 2>/dev/null; then
        if kill -0 "$kpid" 2>/dev/null; then
            kill "$kpid" 2>/dev/null
        fi
        wait "$kpid" 2>/dev/null
        qg_kill_group "$pid"
        wait "$pid" 2>/dev/null
        QG_LAST_RC=124
        qg_guard_note boot_end
        return 124
    fi
    wait "$pid" 2>/dev/null
    QG_LAST_RC=$?
    qg_guard_note boot_end
    return 0
}

# --------------------------------------------------- cổng danh tính + bất biến (D2-80, KT-408e)
# **KT-408e** (D2-263) — boot dò **một** lần theo **đúng** khuôn `soak-x86_64.sh:1403-1408`:
# dùng **cùng** đường bơm phím `monitor-sendkey.py` như mọi boot soak (ảnh aarch64 có TAP 32
# `input: key` phụ thuộc phím ⇒ boot dò bằng `qg_boot` trần sẽ in `not ok 32`), rồi chấm bằng
# **chính** `check_boot_log` của soak. Mọi lệch TAP/bất biến ⇒ `exit 3` (`TỪ CHỐI ĐO`) ở **boot
# 0** sau ~2 s, **không** đốt 1.000 boot. Bằng chứng Peak 2 (`becc1756`): lệch `1..46` ↔ 50 làm
# đỏ **1.000/1.000** boot trong **971 s** — đúng lỗ hổng (ii) của `D2-263`.
echo "# probe (D2-80): boot MỘT lần để đọc dòng '# conf image:' + chấm mọi bất biến của một boot sạch"
if ! boot_keys "$PROBE_LOG" "$SOCKDIR/mon-probe.sock" 0; then
    echo "# soak-aarch64.sh: TỪ CHỐI ĐO — probe boot không thoát/chưa chạy sạch (rc=$QG_LAST_RC, pid=$QG_LAST_PID, log=$PROBE_LOG)" >&2
    exit 3
fi
PROBE_BUILD="$(sed -n 's/^# conf image: build=\([0-9a-f]*\) text=.*/\1/p' "$PROBE_LOG" | head -1)"
PROBE_TEXT="$(sed -n 's/^# conf image: build=.* text=\([0-9]*\)$/\1/p' "$PROBE_LOG" | head -1)"
if [ "$PROBE_BUILD" != "$WANT_BUILD" ] || [ "$PROBE_TEXT" != "$WANT_TEXT" ]; then
    echo "# soak-aarch64.sh: TỪ CHỐI ĐO — ảnh in '# conf image: build=${PROBE_BUILD:-none} text=${PROBE_TEXT:-none}', ELF là build=$WANT_BUILD text=$WANT_TEXT (D2-80)" >&2
    echo "# soak-aarch64.sh: ảnh KHÔNG tự khai hash khớp ELF ⇒ không có neo, không đo (xem k4-aarch64-boot.md §7)" >&2
    exit 3
fi
echo "# probe: '# conf image:' khớp ELF (build=$WANT_BUILD text=$WANT_TEXT)"
if ! PROBE_WHY="$(check_boot_log "$PROBE_LOG" "$WANT_BUILD" "$WANT_TEXT" \
    "probe: harness đã mở log trước khi chạy QEMU; qemu-rc=$QG_LAST_RC" 2>&1)"; then
    echo "# soak-aarch64.sh: TỪ CHỐI ĐO — probe boot không đạt bất biến của một boot sạch:" >&2
    printf '%s\n' "$PROBE_WHY" | sed 's/^/#   /' >&2
    exit 3
fi
echo "# probe: log đạt mọi bất biến của một boot sạch (TAP $TAP_TESTS/$TAP_TESTS + marker + halt)"

# `boot_one <i>`: chạy 1 boot, kiểm **mọi** bất biến, ghi `ok i` hoặc `fail i <lý do>`. Trả 0/1.
boot_one() {
    local i="$1" out="$RUNDIR/boot-$i.log" why="" rc=0
    boot_keys "$out" "$SOCKDIR/mon-$i.sock" "$i"
    rc=$?
    case "$rc" in
        0) ;;
        124)
            printf 'fail %s timeout-%ss\n' "$i" "$((TIMEOUT_DS / 10))" >>"$RESULTS"
            echo "boot $i/$N FAIL: quá $((TIMEOUT_DS / 10)) s (treo) pid=$QG_LAST_PID log=$out" >&2
            return 1
            ;;
        125)
            # **KT-348b** — mất log = mất phép kiểm ⇒ **fail-closed có tên** và **dừng soak**:
            # chạy tiếp chỉ sinh thêm boot không có bằng chứng (KT-348: 7 boot 814..823 mất log
            # vì đĩa đầy, harness vẫn chạy tiếp và báo "không có log" không lý do).
            printf 'fail %s start-%s\n' "$i" "$(soak_reason "$QG_START_ERR")" >>"$RESULTS"
            printf '#   hạ tầng: boot %s: QEMU không khởi động được: %s\n' "$i" "$QG_START_ERR" \
                >>"$INFRA_FAIL"
            echo "boot $i/$N FAIL: hạ tầng — không khởi động được QEMU: $QG_START_ERR (dừng soak, fail-closed)" >&2
            : >"$INFRA_STOP"
            return 1
            ;;
        126)
            printf 'fail %s monitor-%s\n' "$i" "$(soak_reason "$QG_KEYS_ERR")" >>"$RESULTS"
            printf '#   hạ tầng: boot %s: không gửi được phím: %s\n' "$i" "$QG_KEYS_ERR" \
                >>"$INFRA_FAIL"
            echo "boot $i/$N FAIL: harness — không gửi được 'sendkey a': $QG_KEYS_ERR (KHÔNG phải lỗi nhân; T-INFRA)" >&2
            return 1
            ;;
        *)
            printf 'fail %s start-unknown-rc%s\n' "$i" "$rc" >>"$RESULTS"
            printf '#   hạ tầng: boot %s: mã trả lạ %s\n' "$i" "$rc" >>"$INFRA_FAIL"
            echo "boot $i/$N FAIL: hạ tầng — mã trả lạ $rc (dừng soak, fail-closed)" >&2
            : >"$INFRA_STOP"
            return 1
            ;;
    esac

    why="$(check_boot_log "$out" "$WANT_BUILD" "$WANT_TEXT" \
        "harness đã mở log trước khi chạy QEMU; qemu-rc=$QG_LAST_RC" 2>&1)" || {
        printf 'fail %s %s\n' "$i" "$(printf '%s' "$why" | tr '\n' ';' | tr ' ' ',')" >>"$RESULTS"
        echo "boot $i/$N FAIL: $why log=$out" >&2
        tail -5 "$out" | sed 's/^/    /' >&2
        return 1
    }
    printf 'ok %s\n' "$i" >>"$RESULTS"
    echo "boot $i/$N ok"
    return 0
}

# **KT-348b** — dừng các lane khi cờ hạ tầng bật (fail-closed: đĩa/log hỏng thì không chạy tiếp
# 1.000 boot không bằng chứng).
boot_one_or_stop() {
    [ -f "$INFRA_STOP" ] && return 0
    boot_one "$@"
}

# ------------------------------------------------------------------ luồng
# **KT-347b** — vòng luồng ở **một** nguồn `lib/soak-common.sh`; `wait` **đúng PID từng luồng**
# (KHÔNG `wait` trần — sẽ chờ cả TCP server nền, bài học KT-289).
soak_common_run_lanes "$N" "$LANES" boot_one_or_stop
WALL="$SOAK_WALL"

# **KT-348b** — lỗi **hạ tầng** in ngay, có tên (không để chìm trong danh sách boot đỏ), rồi
# **đếm tệp log**: thiếu log = mất bằng chứng = soak ĐỎ (fail-closed). Đây là hai phép kiểm còn
# thiếu ở KT-348 (7 boot 814..823 mất log vì `No space left on device` mà không ai đếm).
SOAK_ABORTED=0
if [ -f "$INFRA_STOP" ]; then
    SOAK_ABORTED=1
    echo "# soak-aarch64.sh: hạ tầng — DỪNG SOAK sớm (fail-closed): có boot không mở được log/không khởi động được QEMU"
fi
if [ -s "$INFRA_FAIL" ]; then
    echo "# soak-aarch64.sh: hạ tầng — danh sách lỗi có tên:"
    sed 's/^/# /' "$INFRA_FAIL"
fi
SOAK_LOGS_BAD=0
soak_a64_log_inventory "$N" "$RUNDIR" || SOAK_LOGS_BAD=1
# **KT-348b** — bằng chứng gửi phím (đường gửi đã rút ngắn: nối monitor trước `ready`).
KBD_OK="$(grep -c ' ok ready_ms=' "$KBD_LOG" 2>/dev/null || true)"
KBD_FAIL="$(grep -c ' fail reason=' "$KBD_LOG" 2>/dev/null || true)"
KBD_READY_MAX="$(sed -n 's/.* ok ready_ms=\([0-9][0-9]*\).*/\1/p' "$KBD_LOG" 2>/dev/null | sort -n | tail -1)"
KBD_SEND_MAX="$(sed -n 's/.* send_ms=\([0-9][0-9]*\).*/\1/p' "$KBD_LOG" 2>/dev/null | sort -n | tail -1)"
echo "# kbd: sendkey ok=${KBD_OK:-0} fail=${KBD_FAIL:-0} ready_ms_max=${KBD_READY_MAX:-0} send_ms_max=${KBD_SEND_MAX:-0} (log $KBD_LOG)"
if [ "${KBD_FAIL:-0}" -ne 0 ]; then
    echo "# kbd: các boot gửi phím hỏng (lý do có tên — T-INFRA, KHÔNG phải lỗi nhân):"
    grep ' fail reason=' "$KBD_LOG" 2>/dev/null | sed 's/^/#   /'
fi

# **KT-289** — dừng server TCP và in bằng chứng **hai chiều** tổng hợp: mỗi boot (kể cả boot dò)
# phải để lại `accept` + `sent=13` + `recv=15`. Không đủ số kết nối nghĩa là có boot không chạy
# tới TCP — boot đó đã ĐỎ ở `check_boot_log`, đây chỉ là bằng chứng phụ (không nới ngưỡng nào).
stop_soak_tcp_server
TCP_CONNS="$(grep -c '^accept ' "$TCP_SERVER_LOG" 2>/dev/null || true)"
TCP_SENT="$(grep -c "^sent=$TCP_TOKEN_LEN$" "$TCP_SERVER_LOG" 2>/dev/null || true)"
TCP_RECV="$(grep -c "^recv=$TCP_SEND_LEN$" "$TCP_SERVER_LOG" 2>/dev/null || true)"
echo "# tcp server: connections=${TCP_CONNS:-0} sent${TCP_TOKEN_LEN}=${TCP_SENT:-0} recv${TCP_SEND_LEN}=${TCP_RECV:-0} (mỗi boot một kết nối; log $TCP_SERVER_LOG)"

# **KT-300** — dừng server DNS và kiểm **fail-closed** rằng DNS của **mọi** boot đi server cục bộ:
#   (a) log server có ≥ N+1 truy vấn (probe + N boot) và ≥ N+1 câu trả lời;
#   (b) mỗi log boot (kể cả probe) có `# net: dns example.com a=192.0.2.123 ttl=3600` — địa chỉ
#       TEST-NET-1 mà resolver thật **không** trả ⇒ interpose hỏng là ĐỎ, không âm thầm quay lại
#       resolver host.
# Đây là **thêm** kiểm (KT-300), không thay/nới kiểm cũ nào.
stop_soak_dns_server
DNS_QUERIES="$(grep -c '^query ' "$DNS_SERVER_LOG" 2>/dev/null || true)"
DNS_REPLIES="$(grep -c '^reply ' "$DNS_SERVER_LOG" 2>/dev/null || true)"
DNS_LOCAL_HITS="$(grep -h -F "# net: dns example.com a=$DNS_ANSWER_IP ttl=$DNS_TTL" \
    "$PROBE_LOG" "$RUNDIR"/boot-*.log 2>/dev/null | wc -l | tr -d ' ')"
DNS_ATTEMPTS1="$(grep -h -c 'attempts=1 const:timeout_ms=3000' \
    "$PROBE_LOG" "$RUNDIR"/boot-*.log 2>/dev/null | awk '{s += $1} END {print s + 0}')"
echo "# dns server: queries=${DNS_QUERIES:-0} replies=${DNS_REPLIES:-0} local_hits=${DNS_LOCAL_HITS:-0} attempts1=${DNS_ATTEMPTS1:-0} (cần ≥ $((N + 1)) truy vấn/trả lời và đúng $((N + 1)) local_hits; log $DNS_SERVER_LOG)"
DNS_BAD=0
# **KT-348b** — soak dừng sớm vì hạ tầng (đĩa/log/QEMU) thì số truy vấn **không thể** đủ N+1; đó
# không phải "interpose hỏng" ⇒ không kết luận DNS, nêu rõ lý do đã dừng (tránh báo sai nguyên nhân).
if [ "${SOAK_ABORTED:-0}" -ne 0 ]; then
    echo "# dns server: KHÔNG kết luận được (soak dừng sớm vì hạ tầng — xem dòng '# soak-aarch64.sh: hạ tầng')"
else
    [ "${DNS_QUERIES:-0}" -ge $((N + 1)) ] || {
        echo "# dns server: chỉ ${DNS_QUERIES:-0} truy vấn < $((N + 1)) (probe + N boot) — interpose KHÔNG có tác dụng?"
        DNS_BAD=1
    }
    [ "${DNS_REPLIES:-0}" -ge $((N + 1)) ] || {
        echo "# dns server: chỉ ${DNS_REPLIES:-0} câu trả lời < $((N + 1)) — server không trả lời đủ?"
        DNS_BAD=1
    }
    [ "${DNS_LOCAL_HITS:-0}" -eq $((N + 1)) ] || {
        echo "# dns server: ${DNS_LOCAL_HITS:-0} log boot có 'a=$DNS_ANSWER_IP ttl=$DNS_TTL', cần đúng $((N + 1)) (probe + N boot)"
        DNS_BAD=1
    }
fi

# ------------------------------------------------------------------ tổng
# **KT-347b** — đếm/neo ảnh/BUILD_ID/kết luận ở **một** nguồn `lib/soak-common.sh`; phép kiểm riêng
# của aarch64 (DNS cục bộ KT-300, đếm log KT-348b) đi qua `SOAK_EXTRA_CHECK` và chạy **trước**
# nhánh SMOKE như cũ.
soak_a64_extra_check() {
    if [ "${SOAK_LOGS_BAD:-0}" -ne 0 ]; then
        echo "# soak-aarch64.sh: FAIL — thiếu tệp log boot (mất bằng chứng; xem dòng '# không có log … (lý do: …)')"
        return 1
    fi
    if [ "${DNS_BAD:-0}" -ne 0 ]; then
        echo "# soak-aarch64.sh: FAIL — DNS cục bộ không được dùng đủ (KT-300, xem dòng '# dns server:')"
        return 1
    fi
    return 0
}
SOAK_N="$N"
SOAK_NEED="$NEED"
SOAK_SMOKE="$SMOKE"
SOAK_TAG="soak-aarch64.sh"
SOAK_SMOKE_MSG="K4'"
SOAK_GIT_HEAD="$GIT_HEAD"
SOAK_IMG_SHA="$IMG_SHA"
SOAK_BUILD_ID="$WANT_BUILD"
SOAK_IMAGE="$IMAGE"
SOAK_RESULTS="$RESULTS"
SOAK_WALL="$WALL"
SOAK_EXTRA_CHECK=soak_a64_extra_check
soak_common_finish
exit $?
