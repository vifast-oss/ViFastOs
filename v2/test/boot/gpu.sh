#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-263 (D2-203) — runner ảnh **test virtio-gpu 2D** riscv64 (`vifast-kbin-ktest-gpu`).
#
# Vì sao runner riêng (không mở rộng `ktest.sh`): ảnh này cần **thiết bị màn hình** trong dòng
# QEMU (`-device virtio-gpu-pci`) **và** điều khiển QEMU từ ngoài **trong lúc khách đang chạy**
# (monitor `screendump`) — nhét vào `ktest.sh` sẽ phải sửa manifest/expected.txt của 21 boot matrix
# cũ và thêm một đường điều khiển QEMU vào runner chung (rủi ro xung đột với các nhánh khác).
#
# # Cách chụp màn hình (ghi rõ theo yêu cầu của task)
#
# QEMU mở **HMP monitor** trên unix socket: `-monitor unix:<sock>,server=on,wait=off`. Runner:
#   1. khởi động QEMU (process group riêng qua `qemu_guard.sh`) với `-device virtio-gpu-pci
#      -display none`; **không** `-icount` (bài test này tương tác — host phải chụp tại một thời
#      điểm thật, nên `rdtime` của khách phải bám đồng hồ thật);
#   2. chờ log có dòng `# gpu: drawn` (ảnh in **sau** `RESOURCE_FLUSH` — chụp sớm là chưa có gì);
#   3. nối socket, đọc banner tới `(qemu) `, gửi `screendump <tệp.ppm>`;
#   4. chờ QEMU tự thoát trong ngân sách.
# Không dùng `sleep` mù: mốc là **marker của khách**, không phải đồng hồ của host.
#
# **`-display none` vẫn chụp được** (đo được trước khi viết driver): console của QEMU tồn tại
# không cần cửa sổ, `screendump` đọc surface của console. Sau `SET_SCANOUT`, QEMU thay surface đó
# bằng **chính resource** của khách (`qemu_console_set_surface`), nên PPM có đúng 640×480 và đúng
# nội dung driver vẽ. Không cần `-vnc`.
#
# # Bằng chứng **thật** là điểm ảnh, không phải TAP
#
# TAP chỉ nói "lệnh gửi xong". `gpu_ppm.py` đọc PPM và đòi **đúng màu tại đúng toạ độ** đã vẽ:
# đỏ `(255,0,0)` trong hình chữ nhật `(40,40,200,120)`, đen ngoài nó, trắng ở vùng chữ `"ViFast"`
# (đúng 122 pixel trắng — tổng `popcount` của sáu glyph 8×8). Nếu driver không vẽ hoặc không
# `TRANSFER_TO_HOST_2D`, PPM vẫn là nền console cũ ⇒ mọi ca PPM đỏ.
#
# Hợp đồng (fail-closed, không nới ngưỡng):
#   1. **Ca dương** (`-device virtio-gpu-pci` + monitor): log phải có `TAP version 13` + `1..5`
#      đúng một lần, năm dòng `ok 1..5` đúng **thứ tự** và đúng **tên**, **không** `not ok`, đúng
#      **một** dòng `PANIC: KT-263 gpu xong (5 TAP, 0 fail)`, và các dòng bằng chứng:
#        - `# gpu pci:` có `vendor=0x1af4 device=0x1050|0x1010` + `assigned=0x40800000
#          readback=0x40800000` (leaf 4 của cửa sổ PCIe, **không** đụng leaf blk/console/net/input);
#        - `# gpu dev:` có `version1=1 device_status=0xf num_scanouts=1`;
#        - `# gpu dma:` có `identity=1`;
#        - `# gpu fb:` có `bytes=1228800 pages=300 physmap=1`;
#        - `# gpu display:` có `w=` và `h=` (mode **thiết bị báo**);
#        - `# gpu resource:` có `id=1 format=1 w=640 h=480 entries=19`;
#        - `# gpu scanout:` có `bogus=InvalidResourceId` (ca âm đối chứng: thiết bị **kiểm**
#          `resource_id`, nên "SET_SCANOUT thành công" không phải lệnh rỗng);
#        - `# gpu draw:` có `rect=40,40,200,120` + `text=ViFast at=40,200`;
#        - `# gpu: drawn` đúng một lần (bằng chứng runner chụp **sau** flush);
#        - `# gpu stats:` có `errors=0 timeouts=0 id_mismatch=0 rejects=1`.
#      **Và** `gpu_ppm.py` phải xanh trên PPM vừa chụp.
#   2. **Ca âm "không thiết bị"**: **cùng** ảnh, **không** `-device` ⇒ log phải có
#      `gpu: no device` + `not ok 1 - gpu device`, **không** có `ok 1`, **không** có dòng PANIC
#      "xong". Runner coi ca âm **đạt** khi ảnh đỏ đúng cách; ảnh PASS giả ⇒ runner **đỏ**.
#   3. `--red-selftest`: chứng minh bộ kiểm **không rỗng** — sửa log tốt (xoá `ok 3`, đổi
#      `bogus`, `assigned`, `identity`, `physmap`, `entries`, `version1`, `device_status`, `rect`,
#      `text`, `errors`, thêm `not ok`, xoá `# gpu: drawn`, xoá `# gpu stats:`, đổi dòng PANIC) và
#      sửa PPM tốt (đổi pixel đỏ thành đen, đổi pixel nền thành đỏ, xoá một pixel trắng của chữ,
#      đổi kích thước, đổi magic) rồi đòi validator **từ chối** từng bản; và log ca âm "PASS giả"
#      phải bị từ chối.
#
# Dùng:
#   bash v2/test/boot/gpu.sh                 # build + ca dương + ca âm (PASS/FAIL)
#   bash v2/test/boot/gpu.sh --red-selftest  # chỉ tự kiểm validator (không build, không QEMU)
#   bash v2/test/boot/gpu.sh --check-log <log> [positive|negative]   # kiểm MỘT log đã có
#   bash v2/test/boot/gpu.sh --check-ppm <ppm>                       # kiểm MỘT PPM đã có
#   KT263_SKIP_BUILD=1 bash v2/test/boot/gpu.sh                      # dùng ảnh có sẵn
#   KT263_LOGDIR=<dir> bash v2/test/boot/gpu.sh                      # nơi để log
# Exit: 0 = ca dương xanh + ca âm đỏ đúng; 1 = có lỗi; 2 = lỗi cấu hình môi trường.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
TARGET="riscv64gc-unknown-none-elf"
KMANIFEST="$V2_ROOT/kernel/Cargo.toml"
IMAGE="${KT263_IMAGE:-$V2_ROOT/kernel/target/$TARGET/release/vifast-kbin-ktest-gpu}"
LOGDIR="${KT263_LOGDIR:-${V2_RUN_TMP:-${TMPDIR:-/tmp}}/kt263-gpu}"
BUDGET="$HERE/budget.sh"
PPM_CHECK="$HERE/gpu_ppm.py"
# KT-242 (P3): LOGDIR riêng theo lần chạy ⇒ tạo trước lần ghi log đầu tiên.
mkdir -p "$LOGDIR" 2>/dev/null || true
TIMEOUT_DS=200 # 20 s/boot (D2-72 Q10), đơn vị 0,1 s
# Số bước 0,1 s chờ marker `# gpu: drawn` trước khi coi là ảnh không lên được.
READY_DS=100
QEMU_MIN_MAJOR=9
QEMU_MIN_MINOR=2
# Số TAP của ảnh — runner đòi đúng năm dòng `ok 1..5` đúng **tên** (thêm/bớt TAP phải sửa cả
# kernel, tệp này và `# gpu stats` trong cùng commit).
TAP_COUNT=5
# Dòng kết thúc chủ đích của ảnh (khớp `ktest_gpu.rs`).
PANIC_OK='^PANIC: KT-263 gpu xong \(5 TAP, 0 fail\)$'
# Địa chỉ BAR driver **gán** cho gpu = leaf thứ 5 của cửa sổ PCIe (`PCI_MMIO_WINDOW_BASE` +
# 4 × 2 MiB). Đây là **hằng thiết kế** của driver (không phải số đo), runner đối chiếu để bắt
# "gán nhầm leaf của blk/console/net/input".
BAR_ASSIGNED='0x40800000'
# Socket monitor — tên ngắn (giới hạn 104 byte của unix socket trên macOS).
SOCK="${TMPDIR:-/tmp}/kt263-mon-$$.sock"

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

FAILS=0
CHECKS=0

die() {
    echo "gpu.sh: $*" >&2
    exit 2
}

bad() {
    echo "not ok - $*"
    FAILS=$((FAILS + 1))
}

ok() {
    CHECKS=$((CHECKS + 1))
    echo "ok $CHECKS - $*"
}

# KT-78 (OD-K6-6): **mọi** boot đi qua `qemu_guard.sh` — QEMU trong process group riêng, timeout
# giết cả group, trap dọn sạch, watchdog dọn khi cha chết (kể cả SIGKILL).
. "$HERE/qemu_guard.sh"

# ---------------------------------------------------------------- tìm QEMU + OpenSBI
find_qemu() {
    QEMU="$(command -v qemu-system-riscv64 || true)"
    [ -n "$QEMU" ] || die "không có qemu-system-riscv64"
    local version major minor
    version="$("$QEMU" --version 2>/dev/null | head -1 | sed -n 's/.*version \([0-9][0-9]*\)\.\([0-9][0-9]*\).*/\1 \2/p')"
    major="${version%% *}"
    minor="${version##* }"
    case "$major" in '' | *[!0-9]*) die "không đọc được phiên bản QEMU" ;; esac
    if [ "$major" -lt "$QEMU_MIN_MAJOR" ] ||
        { [ "$major" -eq "$QEMU_MIN_MAJOR" ] && [ "$minor" -lt "$QEMU_MIN_MINOR" ]; }; then
        die "QEMU $major.$minor < sàn $QEMU_MIN_MAJOR.$QEMU_MIN_MINOR (D2-72 Q2)"
    fi
    SBI="${OPENSBI:-}"
    if [ -z "$SBI" ]; then
        for c in /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
            /usr/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
            /usr/local/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin; do
            if [ -f "$c" ]; then
                SBI="$c"
                break
            fi
        done
    fi
    [ -f "${SBI:-/nonexistent}" ] || die "không tìm thấy OpenSBI fw_dynamic (đặt OPENSBI=...)"
    PYTHON="$(command -v python3 || true)"
    [ -n "$PYTHON" ] || die "không có python3 (cần để gửi lệnh monitor + kiểm PPM)"
    [ -f "$PPM_CHECK" ] || die "không thấy $PPM_CHECK"
    echo "# qemu $version ($QEMU)"
    echo "# opensbi $SBI"
    echo "# python $PYTHON"
}

# ---------------------------------------------------------------- build
build_image() {
    command -v cargo >/dev/null 2>&1 || die "không có cargo"
    if [ "${KT263_SKIP_BUILD:-0}" != "1" ]; then
        echo "# build ảnh gpu ($TARGET, release, --features ktest,gpu)"
        if ! cargo build --manifest-path "$KMANIFEST" -p vifast-kbin-riscv64 \
            --bin vifast-kbin-ktest-gpu --features ktest,gpu \
            --target "$TARGET" --release >"$LOGDIR/kt263-build-image.log" 2>&1; then
            tail -20 "$LOGDIR/kt263-build-image.log" >&2
            die "build ảnh gpu FAIL"
        fi
    fi
    [ -f "$IMAGE" ] || die "không thấy ảnh $IMAGE"
    # D2-72 Q5: trần `.data+.bss` 262.144 B — ảnh mới cũng chịu trần, không nới.
    if ! bash "$BUDGET" "$IMAGE" ktest-gpu >"$LOGDIR/kt263-budget.log" 2>&1; then
        cat "$LOGDIR/kt263-budget.log" >&2
        die "budget ảnh gpu VƯỢT trần"
    fi
    grep '^# image=' "$LOGDIR/kt263-budget.log" || true
}

# ---------------------------------------------------------------- chụp màn hình qua monitor
# `screendump <tệp.ppm>`: nối **một** phiên HMP, đọc banner tới `(qemu) `, gửi `screendump`, đọc
# prompt. Trả 0 = đã gửi; 1 = không nối được / monitor báo lỗi.
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
    print(f"# gpu.sh: không nối được monitor {sock}: {exc}", file=sys.stderr)
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
    print(f"# gpu.sh: monitor báo lỗi: {out!r}", file=sys.stderr)
    sys.exit(1)
sys.exit(0)
PY
}

# `boot_gpu <log> <ppm>`: khởi động QEMU có màn hình + monitor, chờ marker `# gpu: drawn`, chụp
# màn hình, rồi chờ QEMU tự thoát trong ngân sách.
# Trả 0 = QEMU tự thoát; 124 = quá hạn (đã kill cả group); 125 = không khởi động được.
boot_gpu() {
    local out="$1" ppm="$2"
    rm -f "$SOCK" "$ppm" 2>/dev/null
    qg_tmp "$SOCK"
    qg_start "$out" "$TIMEOUT_DS" "$QEMU" -machine virt,aia=none,aclint=on -bios "$SBI" \
        -kernel "$IMAGE" -nographic -cpu rv64 -m 128M -smp 1 -no-reboot \
        -device virtio-gpu-pci -display none \
        -monitor "unix:$SOCK,server=on,wait=off" || return 125
    local pid="$QG_LAST_PID" waited="${QG_START_SPENT:-0}" i=0
    # 1) chờ marker của **khách** (không sleep mù): ảnh in sau `RESOURCE_FLUSH`.
    while [ "$i" -lt "$READY_DS" ] && kill -0 "$pid" 2>/dev/null; do
        grep -aq '^# gpu: drawn$' "$out" 2>/dev/null && break
        sleep 0.1
        i=$((i + 1))
    done
    if grep -aq '^# gpu: drawn$' "$out" 2>/dev/null; then
        if ! screendump "$ppm"; then
            bad "ca dương: không chụp được màn hình qua monitor $SOCK"
        fi
    else
        bad "ca dương: không thấy marker '# gpu: drawn' sau $((READY_DS / 10)) s"
    fi
    # 2) chờ QEMU tự thoát trong phần ngân sách còn lại (cùng công thức `qg_boot`).
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

# ---------------------------------------------------------------- validator
# Mọi phép kiểm đi qua `bad` (tăng `FAILS` **toàn cục**) và validator trả **hiệu số** ⇒ không có
# đường "kiểm đỏ mà vẫn trả 0" (bẫy đã tự gặp ở bản đầu của `net.sh`).
LOG=""
PPM=""

# `check_tap <số> <tên>`: đòi đúng **một** dòng `ok <số> - <tên>`.
check_tap() {
    local number="$1" name="$2" line count
    line="ok $number - $name"
    count="$(grep -cxF "$line" "$LOG")"
    if [ "$count" != 1 ]; then
        bad "dòng '$line' xuất hiện $count lần (đòi 1)"
    fi
}

# `check_grep <mô tả> <regex>`: đòi **ít nhất một** dòng khớp.
check_grep() {
    local what="$1" pattern="$2"
    if ! grep -qE "$pattern" "$LOG"; then
        bad "không thấy $what (mẫu: $pattern)"
    fi
}

# `check_count <mô tả> <regex> <số đòi>`: đòi **đúng** số dòng khớp.
check_count() {
    local what="$1" pattern="$2" want="$3" count
    count="$(grep -cE "$pattern" "$LOG")"
    if [ "$count" != "$want" ]; then
        bad "$what xuất hiện $count lần (đòi $want)"
    fi
}

# `validate_positive <log> <ppm>`: 0 = log **và** PPM đạt **mọi** điều kiện của ca dương.
validate_positive() {
    local before="$FAILS"
    LOG="$1"
    PPM="$2"
    if [ ! -f "$LOG" ]; then
        bad "ca dương: không có log $LOG"
        return 1
    fi
    # 1) TAP version + kế hoạch, đúng một lần.
    check_count "ca dương: 'TAP version 13'" '^TAP version 13$' 1
    check_count "ca dương: dòng kế hoạch '1..$TAP_COUNT'" "^1\\.\\.$TAP_COUNT\$" 1
    # 2) Năm dòng `ok N - <tên>` đúng thứ tự, đúng tên, mỗi dòng đúng một lần.
    check_tap 1 "gpu device"
    check_tap 2 "gpu display info"
    check_tap 3 "gpu resource"
    check_tap 4 "gpu scanout"
    check_tap 5 "gpu draw"
    # 3) Không `not ok` nào.
    check_count "ca dương: dòng 'not ok'" '^not ok' 0
    # 4) Đúng **một** dòng PANIC kết thúc, đúng mẫu.
    check_count "ca dương: dòng PANIC kết thúc" "$PANIC_OK" 1
    # 5) PCI: vendor/device **đọc từ thiết bị** + BAR gán đúng leaf 4 + đọc lại khớp.
    check_grep "vendor/device thật 'vendor=0x1af4 device=0x1050|0x1010' trong '# gpu pci:'" \
        "^# gpu pci: .*vendor=0x1af4 device=0x(1050|1010)( |\$)"
    check_grep "BAR gán/đọc lại '$BAR_ASSIGNED' trong '# gpu pci:'" \
        "^# gpu pci: .*assigned=$BAR_ASSIGNED readback=$BAR_ASSIGNED( |\$)"
    # 6) Feature/queue/config: chỉ VERSION_1, DRIVER_OK còn, thiết bị báo **có** scanout.
    check_grep "'# gpu dev:' có version1=1 device_status=0xf num_scanouts=1" \
        '^# gpu dev: .*version1=1 .*device_status=0xf .*num_scanouts=1( |$)'
    # 7) DMA identity (ring/lệnh/response nằm trong `.bss` identity-map).
    check_grep "'# gpu dma:' có identity=1" '^# gpu dma: .*identity=1( |$)'
    # 8) Framebuffer: 300 frame, truy cập qua physmap (không map thêm trang).
    check_grep "'# gpu fb:' có bytes=1228800 pages=300 physmap=1" \
        '^# gpu fb: .*bytes=1228800 pages=300 physmap=1( |$)'
    # 9) Mode **thiết bị báo** (giá trị thật, không hardcode trong runner).
    check_grep "'# gpu display:' có w= và h=" '^# gpu display: w=[0-9]+ h=[0-9]+ enabled=[0-9]+'
    # 10) Resource: đúng id/format/cỡ + 19 mảnh backing.
    check_grep "'# gpu resource:' có id=1 format=1 w=640 h=480 entries=19" \
        '^# gpu resource: id=1 format=1 w=640 h=480 entries=19( |$)'
    # 11) Ca âm đối chứng: thiết bị **từ chối** resource rác ⇒ SET_SCANOUT không phải lệnh rỗng.
    check_grep "'# gpu scanout:' có bogus=InvalidResourceId" \
        '^# gpu scanout: .*bogus=InvalidResourceId( |$)'
    # 12) Toạ độ đã vẽ (runner đối chiếu **cùng** hằng với `gpu_ppm.py`).
    check_grep "'# gpu draw:' có rect=40,40,200,120 text=ViFast at=40,200" \
        '^# gpu draw: rect=40,40,200,120 .*text=ViFast at=40,200( |$)'
    # 13) Marker chụp: đúng một lần (runner chụp **sau** flush).
    check_count "ca dương: dòng '# gpu: drawn'" '^# gpu: drawn$' 1
    # 14) Bộ đếm sạch: không lỗi, không timeout, không lệch used->id; đúng **một** lần từ chối
    #     (ca âm đối chứng ở mục 11).
    check_grep "'# gpu stats:' có errors=0 timeouts=0 id_mismatch=0 rejects=1" \
        '^# gpu stats: .*errors=0 timeouts=0 id_mismatch=0 rejects=1( |$)'
    # 15) Ca âm **không** được lẫn vào log ca dương.
    check_count "ca dương: dòng 'gpu: no device'" 'gpu: no device' 0
    # 16) **Bằng chứng thật**: điểm ảnh trong PPM.
    if [ ! -f "$PPM" ]; then
        bad "ca dương: không có ảnh chụp $PPM"
    elif ! "$PYTHON" "$PPM_CHECK" "$PPM" >>"$LOGDIR/kt263-ppm.log" 2>&1; then
        bad "ca dương: PPM không đạt (xem $LOGDIR/kt263-ppm.log)"
    fi
    return $((FAILS - before))
}

# `validate_negative <log>`: 0 = ảnh **đỏ đúng cách** khi không có thiết bị màn hình.
validate_negative() {
    local before="$FAILS"
    LOG="$1"
    if [ ! -f "$LOG" ]; then
        bad "ca âm: không có log $LOG"
        return 1
    fi
    # 1) Ảnh phải in đúng dòng fail-closed.
    check_grep "dòng fail-closed 'gpu: no device'" 'gpu: no device'
    # 2) TAP 1 phải **đỏ**.
    check_count "ca âm: 'not ok 1 - gpu device'" '^not ok 1 - gpu device' 1
    # 3) **Không** được có `ok 1` — đây là chỗ bắt "PASS giả".
    check_count "ca âm: 'ok 1 - gpu device'" '^ok 1 - gpu device' 0
    # 4) **Không** được có dòng kết thúc "xong" (nghĩa là đã đi hết 5 TAP).
    check_count "ca âm: dòng PANIC 'xong'" "$PANIC_OK" 0
    # 5) **Không** được có marker chụp (ảnh dừng trước khi tới đó).
    check_count "ca âm: dòng '# gpu: drawn'" '^# gpu: drawn$' 0
    return $((FAILS - before))
}

# ---------------------------------------------------------------- hai ca
run_positive() {
    local log="$LOGDIR/kt263-positive.log"
    local ppm="$LOGDIR/kt263-positive.ppm"
    echo "# ca dương: -device virtio-gpu-pci -display none + monitor screendump"
    if ! boot_gpu "$log" "$ppm"; then
        bad "ca dương: QEMU không tự thoát trong 20 s (timeout/kill cả group)"
        return 1
    fi
    if validate_positive "$log" "$ppm"; then
        ok "ca dương: 5/5 TAP + PPM đúng màu tại toạ độ đã vẽ ($log, $ppm)"
        return 0
    fi
    return 1
}

run_negative() {
    local log="$LOGDIR/kt263-negative.log"
    echo "# ca âm: cùng ảnh, KHÔNG -device virtio-gpu-pci"
    if ! qg_boot "$log" "$TIMEOUT_DS" "$QEMU" -machine virt,aia=none,aclint=on -bios "$SBI" \
        -kernel "$IMAGE" -nographic -cpu rv64 -m 128M -smp 1 -no-reboot; then
        bad "ca âm: QEMU không tự thoát trong 20 s (timeout/kill cả group)"
        return 1
    fi
    if validate_negative "$log"; then
        ok "ca âm: ảnh in 'gpu: no device' + TAP 1 đỏ, không PASS giả ($log)"
        return 0
    fi
    return 1
}

# ---------------------------------------------------------------- tự kiểm validator
# `expect_accept <mô tả> <hàm> <log> [ppm]` / `expect_reject …`: chạy validator và đòi kết quả.
# Lỗi do **ca cố ý xấu** không tính vào `FAILS` cuối (chỉ tính khi kỳ vọng sai).
# **Bẫy đã tự gặp** (bản đầu): `FAILS="$before"` nuốt luôn cả `bad` của chính kỳ vọng ⇒ selftest
# in `not ok` mà vẫn `exit 0` ("validator rỗng" đúng nghĩa). Nay kỳ vọng sai tăng `SELFTEST_BAD`
# **riêng**, chỉ số lỗi do validator sinh ra mới được hoàn lại.
SELFTEST_BAD=0
expect_accept() {
    local what="$1" fn="$2" log="$3" ppm="${4:-}" before="$FAILS"
    if "$fn" "$log" "$ppm" >/dev/null 2>&1; then
        ok "selftest: $what được chấp nhận"
    else
        CHECKS=$((CHECKS + 1))
        echo "not ok - selftest: $what bị TỪ CHỐI (validator quá chặt/sai)"
        SELFTEST_BAD=$((SELFTEST_BAD + 1))
    fi
    FAILS="$before"
}

expect_reject() {
    local what="$1" fn="$2" log="$3" ppm="${4:-}" before="$FAILS"
    if "$fn" "$log" "$ppm" >/dev/null 2>&1; then
        CHECKS=$((CHECKS + 1))
        echo "not ok - selftest: $what KHÔNG bị bắt (validator rỗng)"
        SELFTEST_BAD=$((SELFTEST_BAD + 1))
    else
        ok "selftest: $what bị từ chối"
    fi
    FAILS="$before"
}

# `make_good_ppm <tệp>`: dựng PPM **tốt** bằng Python (không cần QEMU) — nền đen, hình chữ nhật
# đỏ `(40,40,200,120)`, chữ "ViFast" trắng tại `(40,200)` bằng **sáu glyph 8×8** của bảng font
# public domain (chỉ 6 glyph cần cho fixture; bảng đầy đủ ở `vifast_virtio::gpu::FONT_8X8`).
# Đây là **fixture để kiểm validator**, không phải bằng chứng KT-263.
make_good_ppm() {
    local path="$1"
    "$PYTHON" - "$path" <<'PY'
import sys

path = sys.argv[1]
W, H = 640, 480
RECT = (40, 40, 200, 120)
TEXT_X, TEXT_Y = 40, 200
GLYPHS = {
    "V": [0x33, 0x33, 0x33, 0x33, 0x33, 0x1E, 0x0C, 0x00],
    "i": [0x0C, 0x00, 0x0E, 0x0C, 0x0C, 0x0C, 0x1E, 0x00],
    "F": [0x7F, 0x46, 0x16, 0x1E, 0x16, 0x06, 0x0F, 0x00],
    "a": [0x00, 0x00, 0x1E, 0x30, 0x3E, 0x33, 0x6E, 0x00],
    "s": [0x00, 0x00, 0x3E, 0x03, 0x1E, 0x30, 0x1F, 0x00],
    "t": [0x08, 0x0C, 0x3E, 0x0C, 0x0C, 0x2C, 0x18, 0x00],
}
buf = bytearray(W * H * 3)
rx, ry, rw, rh = RECT
for y in range(ry, ry + rh):
    for x in range(rx, rx + rw):
        o = (y * W + x) * 3
        buf[o : o + 3] = b"\xff\x00\x00"
for index, ch in enumerate("ViFast"):
    bits = GLYPHS[ch]
    for row in range(8):
        for col in range(8):
            if bits[row] >> col & 1:
                o = ((TEXT_Y + row) * W + TEXT_X + index * 8 + col) * 3
                buf[o : o + 3] = b"\xff\xff\xff"
with open(path, "wb") as handle:
    handle.write(b"P6\n%d %d\n255\n" % (W, H))
    handle.write(bytes(buf))
PY
}

# `mutate_ppm <nguồn> <đích> <kiểu>`: sửa **một** điểm ảnh / header để đòi validator từ chối.
mutate_ppm() {
    local src="$1" dst="$2" kind="$3"
    "$PYTHON" - "$src" "$dst" "$kind" <<'PY'
import sys

src, dst, kind = sys.argv[1], sys.argv[2], sys.argv[3]
data = bytearray(open(src, "rb").read())
# Header của fixture luôn là "P6\n640 480\n255\n" (15 byte) — offset dữ liệu = 15.
OFF = len(b"P6\n640 480\n255\n")
W = 640


def put(x, y, rgb):
    o = OFF + (y * W + x) * 3
    data[o : o + 3] = bytes(rgb)


if kind == "red_thanh_den":
    # (140,100) là điểm "giữa" mà `gpu_ppm.py` kiểm — đột biến phải rơi **đúng** điểm đã kiểm,
    # nếu không thì "bị từ chối" chỉ là may mắn.
    put(140, 100, (0, 0, 0))
elif kind == "nen_thanh_do":
    # (639,479) là "góc xa" mà `gpu_ppm.py` kiểm.
    put(639, 479, (255, 0, 0))
elif kind == "xoa_pixel_trang":
    put(40, 200, (0, 0, 0))
elif kind == "sai_kich_thuoc":
    data[3:12] = b"320 240\n\x32"
elif kind == "sai_magic":
    data[0:2] = b"P3"
elif kind == "thieu_du_lieu":
    data = data[: OFF + 100]
else:
    raise SystemExit(f"kiểu đột biến không biết: {kind}")
open(dst, "wb").write(bytes(data))
PY
}

red_selftest() {
    local good="$LOGDIR/kt263-selftest-good.log"
    local neg="$LOGDIR/kt263-selftest-neg.log"
    local tmp="$LOGDIR/kt263-selftest-mut.log"
    local goodppm="$LOGDIR/kt263-selftest-good.ppm"
    local tmpppm="$LOGDIR/kt263-selftest-mut.ppm"
    # Log **tốt** dựng tay: đúng năm TAP + đủ bằng chứng (không phải log QEMU thật — chỉ để kiểm
    # validator, không dùng làm bằng chứng KT-263).
    cat >"$good" <<'EOF'
TAP version 13
1..5
# gpu image: hart=0 stvec=0x802000a0 direct=1 satp=0x80000000000804e8 mode=8 tables=10 used=40960 const:wait_ms=2000 const:fb=640x480 const:fb_pages=300 const:attach_entries=19
# gpu pci: functions=2 bus=0 dev=1 func=0 vendor=0x1af4 device=0x1050 bar=4 kind=mem64 size=16384 assigned=0x40800000 readback=0x40800000 command=0x6
# gpu cap: common=0x0 notify=0x3000 device=0x2000 isr=0x1000 mul=4 cap_bar=4 ecam=0x30008000 ecam_identity=1 bar_identity=1 cap_ranges_ok=1 overlap_checked=1 other_bars=0 reserved_ranges=5
# gpu dev: num_queues=2 queue_max=64 queue_size=16 device_features=0x10130000002 driver_features=0x100000000 version1=1 device_status=0xf num_scanouts=1 num_capsets=0
# gpu dma: desc=0x80209000 avail=0x80209100 used=0x80209128 cmd=0x80209200 resp=0x80209600 identity=1 notify_off=0 notify_addr=0x40803000 queue=0
# gpu fb: pa=0x81000000 bytes=1228800 pages=300 physmap=1 stride=2560
ok 1 - gpu device
# gpu display: w=1280 h=800 enabled=1 scanouts=1
ok 2 - gpu display info
# gpu resource: id=1 format=1 w=640 h=480 entries=19 chunk=65536
ok 3 - gpu resource
# gpu scanout: id=0 resource=1 w=640 h=480 host_w=1280 host_h=800 enabled=1 bogus=InvalidResourceId
ok 4 - gpu scanout
# gpu draw: rect=40,40,200,120 color=0xff0000 text=ViFast at=40,200 fg=0xffffff bg=0x000000 stride=2560
ok 5 - gpu draw
# gpu: drawn
# gpu stats: commands=8 notifies=8 errors=0 timeouts=0 id_mismatch=0 rejects=1 published=8 queue_size=16 device_status=0xf
PANIC: KT-263 gpu xong (5 TAP, 0 fail)
EOF
    make_good_ppm "$goodppm"
    expect_accept "log tốt + PPM tốt dựng tay" validate_positive "$good" "$goodppm"
    # Mười bốn đột biến **log** phải bị **từ chối** — nếu không, validator rỗng.
    local name
    for name in xoa_ok3 doi_bogus doi_assigned doi_identity doi_physmap doi_entries \
        doi_version1 doi_status doi_rect doi_text doi_errors them_not_ok xoa_drawn \
        xoa_stats doi_panic; do
        case "$name" in
            xoa_ok3) grep -v '^ok 3 - gpu resource$' "$good" >"$tmp" ;;
            doi_bogus) sed 's/bogus=InvalidResourceId/bogus=accepted/' "$good" >"$tmp" ;;
            doi_assigned) sed 's/assigned=0x40800000/assigned=0x40000000/' "$good" >"$tmp" ;;
            doi_identity) sed 's/identity=1/identity=0/' "$good" >"$tmp" ;;
            doi_physmap) sed 's/physmap=1/physmap=0/' "$good" >"$tmp" ;;
            doi_entries) sed 's/entries=19/entries=1/' "$good" >"$tmp" ;;
            doi_version1) sed 's/version1=1/version1=0/' "$good" >"$tmp" ;;
            doi_status) sed 's/device_status=0xf/device_status=0x0/' "$good" >"$tmp" ;;
            doi_rect) sed 's/rect=40,40,200,120/rect=0,0,200,120/' "$good" >"$tmp" ;;
            doi_text) sed 's/text=ViFast/text=XXXXXX/' "$good" >"$tmp" ;;
            doi_errors) sed 's/errors=0/errors=1/' "$good" >"$tmp" ;;
            them_not_ok) sed 's/^ok 2 - gpu display info$/not ok 2 - gpu display info/' "$good" >"$tmp" ;;
            xoa_drawn) grep -v '^# gpu: drawn$' "$good" >"$tmp" ;;
            xoa_stats) grep -v '^# gpu stats:' "$good" >"$tmp" ;;
            doi_panic) sed 's/(5 TAP, 0 fail)/(5 TAP, 1 fail)/' "$good" >"$tmp" ;;
        esac
        expect_reject "đột biến log '$name'" validate_positive "$tmp" "$goodppm"
    done
    # Sáu đột biến **PPM** phải bị **từ chối** (log tốt, ảnh sai).
    for name in red_thanh_den nen_thanh_do xoa_pixel_trang sai_kich_thuoc sai_magic thieu_du_lieu; do
        mutate_ppm "$goodppm" "$tmpppm" "$name"
        expect_reject "đột biến PPM '$name'" validate_positive "$good" "$tmpppm"
    done
    # Ca âm "không thiết bị": log đỏ đúng cách ⇒ chấp nhận; log "PASS giả" (có `ok 1`) ⇒ từ chối.
    cat >"$neg" <<'EOF'
TAP version 13
1..5
# gpu pci: err=NoDevice stage=nodevice
# gpu: no device
not ok 1 - gpu device: không tìm/khởi tạo được PCI
# ktest halt
EOF
    expect_accept "log ca âm 'không thiết bị' đỏ đúng cách" validate_negative "$neg"
    sed 's/^not ok 1 - gpu device.*$/ok 1 - gpu device/' "$neg" >"$tmp"
    expect_reject "ca âm 'không thiết bị' PASS giả (có 'ok 1')" validate_negative "$tmp"
    # Ca âm "PASS giả" thứ hai: có `ok 1` **và** dòng PANIC "xong" ⇒ vẫn phải bị từ chối.
    {
        cat "$neg"
        echo "PANIC: KT-263 gpu xong (5 TAP, 0 fail)"
    } >"$tmp"
    expect_reject "ca âm có dòng PANIC 'xong'" validate_negative "$tmp"
    rm -f "$tmp" "$good" "$neg" "$goodppm" "$tmpppm"
    # Kỳ vọng sai của selftest **phải** làm đỏ lần chạy (không bị `FAILS="$before"` nuốt).
    FAILS=$((FAILS + SELFTEST_BAD))
}

# ---------------------------------------------------------------- main
MODE=run
CHECK_LOG=""
CHECK_KIND=positive
CHECK_PPM=""
case "${1:-}" in
    --red-selftest) MODE=selftest ;;
    --check-log)
        MODE=check
        CHECK_LOG="${2:-}"
        CHECK_KIND="${3:-positive}"
        [ -n "$CHECK_LOG" ] || die "--check-log cần đường dẫn log"
        ;;
    --check-ppm)
        MODE=checkppm
        CHECK_PPM="${2:-}"
        [ -n "$CHECK_PPM" ] || die "--check-ppm cần đường dẫn PPM"
        ;;
    "" | --run) MODE=run ;;
    -h | --help)
        sed -n '2,80p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    *) die "tham số không biết: $1" ;;
esac

if [ "$MODE" = selftest ]; then
    PYTHON="$(command -v python3 || true)"
    [ -n "$PYTHON" ] || die "không có python3 (cần để dựng fixture PPM)"
    [ -f "$PPM_CHECK" ] || die "không thấy $PPM_CHECK"
    red_selftest
    echo "# gpu.sh --red-selftest: $CHECKS kiểm, $FAILS lỗi"
    [ "$FAILS" = 0 ] || exit 1
    exit 0
fi

if [ "$MODE" = check ]; then
    case "$CHECK_KIND" in
        positive) validate_positive "$CHECK_LOG" "${3:-}" ;;
        negative) validate_negative "$CHECK_LOG" ;;
        *) die "--check-log: kiểu phải là positive|negative" ;;
    esac
    rc=$?
    echo "# gpu.sh --check-log $CHECK_KIND $CHECK_LOG: rc=$rc"
    exit "$rc"
fi

if [ "$MODE" = checkppm ]; then
    PYTHON="$(command -v python3 || true)"
    [ -n "$PYTHON" ] || die "không có python3"
    "$PYTHON" "$PPM_CHECK" "$CHECK_PPM"
    exit $?
fi

find_qemu
build_image
QG_LOG="$LOGDIR/qemu-guard.log"
rm -f "$QG_LOG" 2>/dev/null # log guard **mới** mỗi lần chạy (tổng hợp theo run)
qg_init || die "qemu_guard: không khởi tạo được (thiếu perl?) — OD-K6-6"

run_positive || true
run_negative || true
qg_infra "kt263-gpu" "$LOGDIR/kt263-infra.log" || true

echo "# gpu.sh: $CHECKS kiểm đạt, $FAILS lỗi (log: $LOGDIR)"
if [ "$FAILS" != 0 ]; then
    exit 1
fi
exit 0
