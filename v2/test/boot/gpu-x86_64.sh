#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-337 (D2-231/D2-239) — runner **virtio-gpu 2D trên x86_64**: boot ảnh
# `vifast-kbin-x86_64 --features gpu` với `-device virtio-gpu-pci`, chụp `screendump` PPM **trong
# lúc khách giữ khung hình**, rồi chạy `gpu_ppm.py` (nguyên bản của riscv64/aarch64) để kiểm **từng
# điểm ảnh**.
#
# # Vì sao runner riêng
#
# Ảnh `run-x86_64.sh` chạy **34 TAP** và không có màn hình; ảnh `--features gpu` thêm **một** ca
# (`gpu: 2d ppm`, TAP 35) và giữ khung 500 ms ở **cuối** boot để runner chụp. Tách runner thì
# `run-x86_64.sh` (34 TAP, 34 ca âm) **không đổi** — cùng nguyên tắc `gpu.sh` của riscv64.
#
# # Cách chụp (không `sleep` mù)
#
# QEMU mở HMP monitor trên unix socket. Runner:
#   1. build ảnh release `--features gpu`;
#   2. khởi động QEMU (`-M q35 -smp 4` + `virtio-net-pci` + `virtio-blk-pci` trên đĩa 8 sector có
#      chữ ký `VIFASTOS` + `virtio-gpu-pci` + `-display none` + monitor) qua `qemu_guard.sh`;
#   3. chờ **marker của khách** `# gpu: console done lines=3` rồi `screendump <ppm>` — ảnh giữ
#      khung `GPU_HOLD_MS` = 500 ms, nên không cần `sleep` mù;
#   4. chờ QEMU tự thoát (marker + halt sạch), rồi kiểm log + PPM.
#
# # Hợp đồng (fail-closed, không nới ngưỡng)
#
#   - `TAP version 13` + `1..37` đúng một lần, đủ **37** dòng `ok 1..37` đúng thứ tự, **0** dòng
#     `not ok`; TAP 35 tên `gpu: 2d ppm`;
#   - marker `# viFastOs x86_64: boot ok` đúng một lần + `# viFastOs x86_64: halt`;
#   - các dòng bằng chứng `# gpu pci:`, `# gpu dev:`, `# gpu dma:`, `# gpu fb:`, `# gpu display:`,
#     `# gpu resource:`, `# gpu scanout:`, `# gpu draw:`, `# gpu console:`, `# gpu stats:` +
#     `# gpu: drawn`, `# gpu: console done lines=3` — mỗi dòng đúng **một** lần và có trường đo;
#   - `gpu_ppm.py` PASS (đỏ ⇒ runner FAIL).
#
# Dùng:
#   bash v2/test/boot/gpu-x86_64.sh                 # build + ca dương (PASS/FAIL)
#   bash v2/test/boot/gpu-x86_64.sh --negative     # ca dương + ca âm "ảnh KHÔNG feature gpu ⇒ ĐỎ"
#   KT337_NEGATIVE=1 bash v2/test/boot/gpu-x86_64.sh   # như trên qua env (cổng T2 dùng)
#   KT337_SKIP_BUILD=1 bash v2/test/boot/gpu-x86_64.sh
#   KT337_LOGDIR=<dir> bash v2/test/boot/gpu-x86_64.sh   # KT_LOGDIR cũng được
#   bash v2/test/boot/gpu-x86_64.sh --check-log <log> <ppm>   # kiểm log+PPM đã có
#   bash v2/test/boot/gpu-x86_64.sh --print-logdir            # in logdir đã giải rồi thoát
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
# **KT-333d (D2-248 mục 2b)** — LOGDIR (và SOCK ngắn) dùng chung `v2/test/boot/lib/env.sh`
# (KT-333c đã làm logdir theo phiên inline ở 6 runner; nay gom một chỗ, không lặp code).
. "$HERE/lib/env.sh"
REPO_ROOT="$(cd "$V2_ROOT/.." && pwd)"
TARGET="x86_64-unknown-none"
# **KT-333c** (D2-243 mục 3b) — logdir **riêng theo phiên** (`$V2_RUN_TMP` hoặc
# `$TMPDIR/kt337-gpu-x86_64.<pid>`); ép bằng `KT_LOGDIR`/`KT337_LOGDIR`.
LOGDIR="$(v2_env_logdir kt337-gpu-x86_64 KT337_LOGDIR)"
if [ "${1:-}" = "--print-logdir" ]; then
    echo "$LOGDIR"
    exit 0
fi
# **KT-333c** — ca âm "không thiết bị GPU" (ảnh không feature `gpu`): chạy thêm sau ca dương.
# Bật bằng `--negative` hoặc `KT337_NEGATIVE=1` (cổng T2 dùng env để một lần chạy kiểm cả hai).
NEGATIVE="${KT337_NEGATIVE:-0}"
if [ "${1:-}" = "--negative" ]; then
    NEGATIVE=1
    shift
fi
IMAGE="$V2_ROOT/kernel/target/$TARGET/release/vifast-kbin-x86_64"
QEMU_DEFAULT="/opt/homebrew/bin/qemu-system-x86_64"
PPM_CHECK="$HERE/gpu_ppm.py"
EXPECT_TESTS=38
BOOT_MARKER='# viFastOs x86_64: boot ok'
HALT_LINE='# viFastOs x86_64: halt'
GPU_MARKER='# gpu: console done lines=3'
# Số bước 0,1 s chờ marker GPU của khách trước khi coi là ảnh không lên được. Boot x86_64 đo
# được ~3 s tới pha GPU; 200 bước = 20 s là dư, không phải ngưỡng thành tích.
READY_DS=200
TIMEOUT_DS=400
QEMU_MACHINE=(-M q35 -cpu "${V2_X64_CPU:-qemu64}" -m 128M -nographic -no-reboot -smp 4 -vga none)
QEMU_NET=(-netdev user,id=n0 -device virtio-net-pci,netdev=n0)

die() {
    echo "gpu-x86_64.sh: $*" >&2
    exit 1
}
bad() {
    echo "not ok - $*"
    FAILS=$((FAILS + 1))
}
ok() {
    echo "ok - $*"
}

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

PYTHON="$(command -v python3 || true)"
[ -n "$PYTHON" ] || die "không có python3"

mkdir -p "$LOGDIR" 2>/dev/null || die "không tạo được $LOGDIR"
LOG="$LOGDIR/kt337-gpu.log"
PPM="$LOGDIR/kt337-gpu.ppm"
# **KT-333c / T2 2026-11-22** — đường dẫn **unix socket** trên macOS **tối đa 104 byte**
# (`Path must be less than 104 bytes`). Từ KT-333c logdir mặc định là `$V2_RUN_TMP` **sâu hơn**
# nhiều (…/v2run.XXXXXX/kt337-gpu-x86_64.<pid>/), ghép thêm tên socket là **vượt** ⇒ QEMU
# **không mở được monitor** ⇒ ca âm đỏ vì lý do SAI. Vì vậy socket đặt ở **thư mục ngắn riêng**
# (`/tmp`), **không** nằm trong logdir; log/PPM vẫn ở `$LOGDIR`.
# **KT-333d (D2-248 mục 2b)** — một chỗ duy nhất (`env.sh`) tính thư mục socket **ngắn**; log/PPM
# vẫn ở `$LOGDIR`. `v2_env_sock` từ chối đường dẫn > 103 byte thay vì để QEMU chết mờ.
SOCKDIR="$(v2_env_sockdir kt337)" || die "không tạo được thư mục socket ngắn"
trap 'rm -rf "$SOCKDIR"' EXIT
SOCK="$(v2_env_sock "$SOCKDIR" mon.sock)" || die "đường dẫn socket quá dài (AF_UNIX < 104 byte)"
# **KT-333c (D2-243 mục 3b)** — ca âm "không thiết bị GPU": ảnh build **không** feature `gpu`
# (nhưng QEMU vẫn có `-device virtio-gpu-pci`) ⇒ bộ kiểm `check_log`/`check_ppm` **phải ĐỎ**.
# Đây là ca âm **thật** (boot QEMU thật) như `run-aarch64.sh` ca (p) / riscv64 `# gpu: no device`.
NEG_LOG="$LOGDIR/kt337-gpu-no-gpu.log"
NEG_PPM="$LOGDIR/kt337-gpu-no-gpu.ppm"
NEG_SOCK="$(v2_env_sock "$SOCKDIR" mon-neg.sock)" || die "đường dẫn socket ca âm quá dài"
if [ "${1:-}" = "--print-sock" ]; then
    echo "$SOCK"
    echo "$NEG_SOCK"
    exit 0
fi
NEG_IMAGE="$LOGDIR/vifast-kbin-x86_64-no-gpu"
NEG_TARGET="$LOGDIR/target-no-gpu"

# ---------------------------------------------------------------- đĩa tạm (KT-288)
DISK="$LOGDIR/disk0.raw"
make_disk() {
    dd if=/dev/zero of="$DISK" bs=512 count=8 2>/dev/null || return 1
    printf 'VIFASTOS-KT288-S0' | dd of="$DISK" bs=1 seek=0 conv=notrunc 2>/dev/null || return 1
    return 0
}
make_disk || die "không tạo được đĩa tạm $DISK"

# ---------------------------------------------------------------- build + ngân sách
if [ "${KT337_SKIP_BUILD:-0}" != 1 ]; then
    # **KT-340a** (D2-08): pha GPU là pha **kiểm** (TAP 35) ⇒ phải build kèm `ktest`.
    echo "# build ảnh KTEST x86_64 ($TARGET, release, --features ktest,gpu)"
    cargo build --manifest-path "$V2_ROOT/kernel/Cargo.toml" --target "$TARGET" \
        --release --bin vifast-kbin-x86_64 --features ktest,gpu \
        >"$LOGDIR/build.log" 2>&1 || die "build FAIL (xem $LOGDIR/build.log)"
fi
[ -f "$IMAGE" ] || die "không có ảnh $IMAGE"
echo "# image=$IMAGE bytes=$(wc -c <"$IMAGE" | tr -d ' ')"
# Trần `.data+.bss` 262.144 B/ảnh (D2-72 Q5) — đo bằng `budget-x86_64.sh` (e_machine=62), không nới.
if ! bash "$HERE/budget-x86_64.sh" "$IMAGE" "x86_64-gpu" >"$LOGDIR/budget.log" 2>&1; then
    cat "$LOGDIR/budget.log"
    die "ngân sách .data+.bss VƯỢT trần (xem $LOGDIR/budget.log)"
fi
grep -E '^# (image=|sum|data|bss)' "$LOGDIR/budget.log" || true

# ---------------------------------------------------------------- chụp màn hình qua monitor
screendump() {
    local path="$1" sock="${2:-$SOCK}"
    "$PYTHON" - "$sock" "$path" <<'PY'
import socket
import sys

sock, path = sys.argv[1], sys.argv[2]
try:
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(5.0)
    s.connect(sock)
except OSError as exc:
    print(f"# gpu-x86_64.sh: không nối được monitor {sock}: {exc}", file=sys.stderr)
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
    print(f"# gpu-x86_64.sh: monitor báo lỗi: {out!r}", file=sys.stderr)
    sys.exit(1)
sys.exit(0)
PY
}

# ---------------------------------------------------------------- một boot + chụp PPM
# Dùng chung cho ca dương (ảnh `gpu`) và ca âm (ảnh **không** `gpu`): cùng đường khởi động, cùng
# ngân sách, cùng monitor ⇒ ca âm chỉ khác **ảnh**, không khác cách đo (nếu khác thì ca âm không
# chứng minh được gì về ca dương).
boot_and_capture() { # <ảnh> <log> <ppm> <socket> [seek_gpu=1]
    local img="$1" log="$2" ppm="$3" sock="$4" seek_gpu="${5:-1}" pid waited i t0 tmax
    rm -f "$log" "$ppm" "$sock" 2>/dev/null
    echo "# boot $(basename "$img") $(wc -c <"$img" | tr -d ' ') B (virtio-net + virtio-blk + virtio-gpu + monitor)"
    qg_start "$log" "$TIMEOUT_DS" "$QEMU" "${QEMU_MACHINE[@]}" -kernel "$img" "${QEMU_NET[@]}" \
        -drive "file=$DISK,if=none,id=d0,format=raw" \
        -device virtio-blk-pci,drive=d0 \
        -device virtio-gpu-pci -display none \
        -monitor "unix:$sock,server=on,wait=off" || return 125
    pid="$QG_LAST_PID"
    waited="${QG_START_SPENT:-0}"
    if [ "$seek_gpu" = "1" ]; then
        i=0
        while [ "$i" -lt "$READY_DS" ] && kill -0 "$pid" 2>/dev/null; do
            grep -aq "$GPU_MARKER" "$log" 2>/dev/null && break
            sleep 0.05
            i=$((i + 1))
        done
        if grep -aq "$GPU_MARKER" "$log" 2>/dev/null; then
            screendump "$ppm" "$sock" || bad "không gửi được lệnh screendump (monitor)"
        else
            bad "không thấy marker '$GPU_MARKER' sau $((READY_DS / 20)) s"
        fi
    fi
    # Chờ QEMU tự thoát (cùng công thức ngân sách của `qg_boot`).
    t0=$SECONDS
    tmax=$(((TIMEOUT_DS / 10) * 2 + 5))
    while kill -0 "$pid" 2>/dev/null && [ "$waited" -lt "$TIMEOUT_DS" ] &&
        [ $((SECONDS - t0)) -lt "$tmax" ]; do
        sleep 0.1
        waited=$((waited + 1))
    done
    if kill -0 "$pid" 2>/dev/null; then
        qg_kill_group "$pid"
        wait "$pid" 2>/dev/null
        bad "QEMU không tự thoát trong ngân sách (đã kill group)"
    else
        wait "$pid" 2>/dev/null || true
    fi
    return 0
}

# ---------------------------------------------------------------- QEMU
QEMU="${KT337_QEMU:-$QEMU_DEFAULT}"
[ -x "$QEMU" ] || QEMU="$(command -v qemu-system-x86_64 || true)"
[ -n "$QEMU" ] || die "không có qemu-system-x86_64"
. "$HERE/qemu_guard.sh"

FAILS=0
qg_init || die "qemu_guard: không khởi tạo được (thiếu perl?) — OD-K6-6"
qg_tmp "$SOCK" "$NEG_SOCK"
echo "# boot x86_64 --features gpu (virtio-net + virtio-blk + virtio-gpu + monitor)"
boot_and_capture "$IMAGE" "$LOG" "$PPM" "$SOCK" || die "không khởi động được QEMU"
qg_infra "kt337-gpu-x86_64" "$LOGDIR/kt337-infra.log" || true

check_log() {
    # TAP: `TAP version 13` + `1..35` + đủ 35 dòng `ok`, 0 `not ok`.
    if [ "$(grep -c '^TAP version 13$' "$LOG" || true)" -ne 1 ]; then
        bad "log phải có đúng một 'TAP version 13'"
    fi
    if [ "$(grep -c "^1\.\.$EXPECT_TESTS$" "$LOG" || true)" -ne 1 ]; then
        bad "log phải có đúng một '1..$EXPECT_TESTS'"
    fi
    local n
    n="$(grep -c '^ok [0-9]\+ - ' "$LOG" || true)"
    if [ "${n:-0}" -ne "$EXPECT_TESTS" ]; then
        bad "log có ${n:-0} dòng 'ok', đòi $EXPECT_TESTS"
    fi
    if grep -q '^not ok' "$LOG"; then
        bad "log có dòng 'not ok'"
        grep '^not ok' "$LOG" | head -5
    fi
    # Đúng thứ tự 1..37.
    local want=1
    while IFS= read -r line; do
        local got
        got="$(printf '%s' "$line" | sed -n 's/^ok \([0-9]\+\) - .*/\1/p')"
        if [ -n "$got" ] && [ "$got" != "$want" ]; then
            bad "TAP số hiệu lệch: thấy '$got', đòi '$want'"
            break
        fi
        [ -n "$got" ] && want=$((want + 1))
    done <"$LOG"
    # TAP 37 đúng tên (KT-340a dời gpu 35→37 sau khi thêm W^X 35/36).
    if ! grep -qx "ok $EXPECT_TESTS - gpu: 2d ppm" "$LOG"; then
        bad "thiếu 'ok $EXPECT_TESTS - gpu: 2d ppm'"
    fi
    # Marker + halt đúng một lần.
    if [ "$(grep -cF "$BOOT_MARKER" "$LOG" || true)" -ne 1 ]; then
        bad "marker '$BOOT_MARKER' phải xuất hiện đúng một lần"
    fi
    if [ "$(grep -cF "$HALT_LINE" "$LOG" || true)" -ne 1 ]; then
        bad "dòng halt '$HALT_LINE' phải xuất hiện đúng một lần"
    fi
    if grep -q 'PANIC' "$LOG"; then
        bad "log có PANIC"
    fi
    # Mỗi dòng bằng chứng GPU đúng **một** lần.
    local prefix
    for prefix in '# gpu pci: ' '# gpu dev: ' '# gpu dma: ' '# gpu fb: ' '# gpu display: ' \
        '# gpu resource: ' '# gpu scanout: ' '# gpu draw: ' '# gpu console: ' '# gpu stats: '; do
        n="$(grep -c "^$prefix" "$LOG" || true)"
        if [ "${n:-0}" -ne 1 ]; then
            bad "dòng '$prefix' phải đúng một lần, thấy ${n:-0}"
        fi
    done
    for want in '# gpu: drawn' "$GPU_MARKER"; do
        if [ "$(grep -cxF "$want" "$LOG" || true)" -ne 1 ]; then
            bad "dòng '$want' phải đúng một lần"
        fi
    done
    # Trường đo bắt buộc trên từng dòng.
    local line
    line="$(grep -E '^# gpu pci: ' "$LOG" || true)"
    for want in 'vendor=0x1af4' 'device=0x1050' 'bar=' 'assigned=0x' 'readback=0x' 'command=0x'; do
        case "$line" in
        *"$want"*) ;;
        *) bad "'# gpu pci:' thiếu '$want' (dòng: $line)" ;;
        esac
    done
    case "$line" in
    *"assigned=0x0 "* | *"assigned=0x "*) bad "'# gpu pci:' BAR chưa được gán (dòng: $line)" ;;
    esac
    line="$(grep -E '^# gpu dev: ' "$LOG" || true)"
    for want in 'version1=1' 'device_status=0x0f' 'num_scanouts=1' 'queue_size=16'; do
        case "$line" in
        *"$want"*) ;;
        *) bad "'# gpu dev:' thiếu '$want' (dòng: $line)" ;;
        esac
    done
    line="$(grep -E '^# gpu dma: ' "$LOG" || true)"
    for want in 'desc=0x' 'avail=0x' 'used=0x' 'cmd=0x' 'resp=0x' 'dma=0x' 'notify_off='; do
        case "$line" in
        *"$want"*) ;;
        *) bad "'# gpu dma:' thiếu '$want' (dòng: $line)" ;;
        esac
    done
    line="$(grep -E '^# gpu fb: ' "$LOG" || true)"
    for want in 'identity=1' 'bytes=1228800' 'pages=300' 'stride=2560' 'attach_entries=19'; do
        case "$line" in
        *"$want"*) ;;
        *) bad "'# gpu fb:' thiếu '$want' (dòng: $line)" ;;
        esac
    done
    line="$(grep -E '^# gpu display: ' "$LOG" || true)"
    case "$line" in
    *'w='*' h='*) ;;
    *) bad "'# gpu display:' thiếu w/h (dòng: $line)" ;;
    esac
    line="$(grep -E '^# gpu resource: ' "$LOG" || true)"
    for want in 'id=1' 'format=1' 'w=640' 'h=480' 'entries=19'; do
        case "$line" in
        *"$want"*) ;;
        *) bad "'# gpu resource:' thiếu '$want' (dòng: $line)" ;;
        esac
    done
    line="$(grep -E '^# gpu scanout: ' "$LOG" || true)"
    for want in 'bogus=InvalidResourceId' 'host_w=' 'enabled='; do
        case "$line" in
        *"$want"*) ;;
        *) bad "'# gpu scanout:' thiếu '$want' (dòng: $line)" ;;
        esac
    done
    line="$(grep -E '^# gpu draw: ' "$LOG" || true)"
    for want in 'rect=40,40,200,120' 'text=ViFast at=40,200'; do
        case "$line" in
        *"$want"*) ;;
        *) bad "'# gpu draw:' thiếu '$want' (dòng: $line)" ;;
        esac
    done
    line="$(grep -E '^# gpu console: ' "$LOG" || true)"
    for want in 'chars=29' 'newlines=3' 'scrolls=0' 'flushes=1' 'commands=2' 'matched=29/29' \
        'text=ViFastOs|gpu console|0123456789'; do
        case "$line" in
        *"$want"*) ;;
        *) bad "'# gpu console:' thiếu '$want' (dòng: $line)" ;;
        esac
    done
    line="$(grep -E '^# gpu stats: ' "$LOG" || true)"
    for want in 'errors=0' 'timeouts=0' 'id_mismatch=0' 'rejects=1' 'device_status=0x0f'; do
        case "$line" in
        *"$want"*) ;;
        *) bad "'# gpu stats:' thiếu '$want' (dòng: $line)" ;;
        esac
    done
}

check_ppm() {
    [ -f "$PPM" ] || {
        bad "không có PPM $PPM (screendump thất bại?)"
        return
    }
    local bytes
    bytes="$(wc -c <"$PPM" | tr -d ' ')"
    echo "# ppm=$PPM bytes=$bytes"
    if ! "$PYTHON" "$PPM_CHECK" "$PPM" >"$LOGDIR/ppm-check.log" 2>&1; then
        bad "gpu_ppm.py ĐỎ (xem $LOGDIR/ppm-check.log)"
        tail -20 "$LOGDIR/ppm-check.log"
        return
    fi
    ok "gpu_ppm.py PASS ($(grep -c '^ok ' "$LOGDIR/ppm-check.log" || true) kiểm)"
}

# ---------------------------------------------------------------- chỉ kiểm log + PPM đã có
if [ "${1:-}" = "--check-log" ]; then
    [ $# -eq 3 ] || die "dùng: --check-log <log> <ppm>"
    LOG="$2"
    PPM="$3"
    [ -f "$LOG" ] || die "không có log $LOG"
    [ -f "$PPM" ] || die "không có PPM $PPM"
    FAILS=0
    check_log
    check_ppm
    echo "# gpu-x86_64.sh --check-log: $FAILS lỗi"
    [ "$FAILS" -eq 0 ] || exit 1
    exit 0
fi

echo "# kiểm log $LOG"
check_log
echo "# kiểm PPM $PPM"
check_ppm

# ---------------------------------------------------------------- ca âm "không GPU" (KT-333c)
# Ảnh build **không** feature `gpu` (nhưng QEMU **vẫn có** `-device virtio-gpu-pci`) ⇒ bộ kiểm
# phải **ĐỎ**. Chỉ chạy khi ca dương xanh (nếu ca dương đỏ thì ca âm không nói thêm gì).
if [ "$NEGATIVE" = "1" ] && [ "$FAILS" -eq 0 ]; then
    echo "# ---------------------------------------------------------------- ca âm: ảnh KHÔNG feature gpu"
    echo "# build ảnh KTEST x86_64 KHÔNG feature gpu (ca âm)"
    if ! cargo build --manifest-path "$V2_ROOT/kernel/Cargo.toml" --target "$TARGET" \
        --release --bin vifast-kbin-x86_64 --features ktest --target-dir "$NEG_TARGET" \
        >"$LOGDIR/build-no-gpu.log" 2>&1; then
        tail -20 "$LOGDIR/build-no-gpu.log" >&2
        bad "ca âm (không GPU): build ảnh không feature gpu FAIL (xem $LOGDIR/build-no-gpu.log)"
    elif ! cp "$NEG_TARGET/$TARGET/release/vifast-kbin-x86_64" "$NEG_IMAGE" 2>/dev/null; then
        bad "ca âm (không GPU): không chép được ảnh không gpu"
    else
        FAILS=0
        boot_and_capture "$NEG_IMAGE" "$NEG_LOG" "$NEG_PPM" "$NEG_SOCK" 0 ||
            bad "ca âm (không GPU): không khởi động được QEMU"
        BOOT_OK=0
        if [ "$(grep -cF "$BOOT_MARKER" "$NEG_LOG" 2>/dev/null || true)" -eq 1 ] &&
            [ "$(grep -cF "$HALT_LINE" "$NEG_LOG" 2>/dev/null || true)" -eq 1 ]; then
            BOOT_OK=1
        fi
        if [ "$BOOT_OK" -ne 1 ]; then
            bad "ca âm (không GPU): ảnh không boot tới marker+halt (ca âm sẽ đỏ vì lý do SAI)"
            echo "# xem $NEG_LOG"
        else
            LOG_SAVE="$LOG"
            PPM_SAVE="$PPM"
            LOG="$NEG_LOG"
            PPM="$NEG_PPM"
            check_log
            if [ -f "$NEG_PPM" ]; then
                check_ppm
            else
                echo "# ca âm: không có PPM (ảnh không feature gpu không dựng framebuffer) — kiểm log là đủ"
            fi
            NFAIL="$FAILS"
            LOG="$LOG_SAVE"
            PPM="$PPM_SAVE"
            if [ "$NFAIL" -gt 0 ]; then
                echo "ok - ca âm (không GPU): bộ kiểm TỪ CHỐI ảnh không feature gpu ($NFAIL lỗi, log $NEG_LOG)"
                FAILS=0
            else
                bad "ca âm (không GPU): bộ kiểm PASS ảnh không feature gpu — **PASS GIẢ**"
            fi
        fi
    fi
fi

echo "# gpu-x86_64.sh: $FAILS lỗi (log: $LOGDIR)"
[ "$FAILS" -eq 0 ] || exit 1
exit 0
