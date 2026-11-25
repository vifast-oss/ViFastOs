#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-255 (D2-200) — runner ảnh **test virtio-input** riscv64 (`vifast-kbin-ktest-input`).
#
# Vì sao runner riêng (không mở rộng `ktest.sh`): ảnh này cần **thiết bị bàn phím** trong dòng
# QEMU (`-device virtio-keyboard-pci`) **và** điều khiển QEMU từ ngoài **trong lúc khách đang chạy**
# (monitor `sendkey`) — nhét vào `ktest.sh` sẽ phải sửa manifest/expected.txt của 21 boot matrix cũ
# và thêm một đường điều khiển QEMU vào runner chung (rủi ro xung đột với các nhánh khác).
#
# # Cách bơm phím (ghi rõ theo yêu cầu của task)
#
# QEMU mở **HMP monitor** trên unix socket: `-monitor unix:<sock>,server=on,wait=off`. Runner:
#   1. khởi động QEMU (process group riêng qua `qemu_guard.sh`), **không** `-icount` (xem dưới);
#   2. chờ log có dòng `# input: ready` (ảnh in **sau** khi đã arm 8 đệm — bơm sớm là mất lô);
#   3. nối socket, đọc banner tới `(qemu) `, gửi `sendkey a`, `sendkey shift-a`, `sendkey f1`;
#   4. chờ QEMU tự thoát trong ngân sách.
# Không dùng `sleep` mù: mốc là **marker của khách**, không phải đồng hồ của host.
#
# Vì sao **không** `-icount`: bài test này **tương tác** — host phải bơm phím tại một thời điểm
# thật, nên `rdtime` của khách phải bám đồng hồ thật (không icount: 1 tick = 1/10.000.000 s thật).
# `-icount` (D2-74) chỉ dùng cho phép đo **drift timer** của `ktest.sh`; ở đây không đo thời gian.
#
# Hợp đồng (fail-closed, không nới ngưỡng):
#   1. **Ca dương** (`-device virtio-keyboard-pci` + monitor): log phải có `TAP version 13` + `1..5`
#      đúng một lần, năm dòng `ok 1..5` đúng **thứ tự** và đúng **tên**, **không** `not ok`, đúng
#      **một** dòng `PANIC: KT-255 input xong (5 TAP, 0 fail)`, và các dòng bằng chứng:
#        - `# input pci:` có `vendor=0x1af4 device=0x1012|0x1052` + `assigned=0x40600000
#          readback=0x40600000` (leaf 3 của cửa sổ PCIe, **không** đụng leaf blk/console/net);
#        - `# input name: QEMU Virtio Keyboard` (tên **đọc từ** `virtio_input_config`);
#        - `# input dev:` có `name_len=20 version1=1 rep_feature=0 key_a_bit=1 device_status=0xf`;
#        - `# input dma:` có `identity=1 armed=8`;
#        - `# input: ready` đúng một lần (bằng chứng runner bơm phím **sau** khi arm);
#        - `# input: key a code=30 ascii=0x61 shift=0`;
#        - `# input: shift code=30 ascii=0x41 shift=1`;
#        - `# input: unknown code=59 ascii=none`;
#        - `# input stats:` có `errors=0 timeouts=0 id_mismatch=0`.
#   2. **Ca âm "không thiết bị"**: **cùng** ảnh, **không** `-device` ⇒ log phải có
#      `input: no device` + `not ok 1 - input device`, **không** có `ok 1`, **không** có dòng PANIC
#      "xong". Runner coi ca âm **đạt** khi ảnh đỏ đúng cách; ảnh PASS giả ⇒ runner **đỏ**.
#   3. `--red-selftest`: chứng minh bộ kiểm **không rỗng** — sửa log tốt (xoá `ok 3`, đổi ASCII,
#      đổi Shift, đổi `ascii=none`, đổi tên, đổi vendor, đổi `armed`, đổi `identity`, thêm
#      `not ok`, xoá `# input stats:`, xoá `# input: ready`) rồi đòi validator **từ chối** từng
#      bản; và log ca âm "PASS giả" phải bị từ chối.
#
# Dùng:
#   bash v2/test/boot/input.sh                 # build + ca dương + ca âm (PASS/FAIL)
#   bash v2/test/boot/input.sh --red-selftest  # chỉ tự kiểm validator (không build, không QEMU)
#   bash v2/test/boot/input.sh --check-log <log> [positive|negative]   # kiểm MỘT log đã có
#   KT255_SKIP_BUILD=1 bash v2/test/boot/input.sh                    # dùng ảnh có sẵn
#   KT255_LOGDIR=<dir> bash v2/test/boot/input.sh                    # nơi để log
# Exit: 0 = ca dương xanh + ca âm đỏ đúng; 1 = có lỗi; 2 = lỗi cấu hình môi trường.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
TARGET="riscv64gc-unknown-none-elf"
KMANIFEST="$V2_ROOT/kernel/Cargo.toml"
IMAGE="${KT255_IMAGE:-${CARGO_TARGET_DIR:-$V2_ROOT/kernel/target}/$TARGET/release/vifast-kbin-ktest-input}"
LOGDIR="${KT255_LOGDIR:-${V2_RUN_TMP:-${TMPDIR:-/tmp}}/kt255-input}"
BUDGET="$HERE/budget.sh"
# KT-242 (P3): LOGDIR riêng theo lần chạy ⇒ tạo trước lần ghi log đầu tiên.
mkdir -p "$LOGDIR" 2>/dev/null || true
TIMEOUT_DS=200 # 20 s/boot (D2-72 Q10), đơn vị 0,1 s
# Số bước 0,1 s chờ marker `# input: ready` trước khi coi là ảnh không lên được.
READY_DS=100
QEMU_MIN_MAJOR=9
QEMU_MIN_MINOR=2
# Số TAP của ảnh — runner đòi đúng năm dòng `ok 1..5` đúng **tên** (thêm/bớt TAP phải sửa cả
# kernel, tệp này và `# input stats` trong cùng commit).
TAP_COUNT=5
# Dòng kết thúc chủ đích của ảnh (khớp `ktest_input.rs`).
PANIC_OK='^PANIC: KT-255 input xong \(5 TAP, 0 fail\)$'
# Tên QEMU đặt cho `virtio-keyboard` (`VIRTIO_ID_NAME_KEYBOARD`, hw/input/virtio-input-hid.c) —
# hằng này chỉ để đối chiếu **nguồn gốc**; tên vẫn là giá trị **đọc từ thiết bị**.
NAME_LINE='^# input name: QEMU Virtio Keyboard$'
# Địa chỉ BAR driver **gán** cho input = leaf thứ 4 của cửa sổ PCIe (`PCI_MMIO_WINDOW_BASE` +
# 3 × 2 MiB). Đây là **hằng thiết kế** của driver (không phải số đo), runner đối chiếu để bắt
# "gán nhầm leaf của blk/console/net".
BAR_ASSIGNED='0x40600000'
# Socket monitor — tên ngắn (giới hạn 104 byte của unix socket trên macOS).
SOCK="${TMPDIR:-/tmp}/kt255-mon-$$.sock"

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

FAILS=0
CHECKS=0

die() {
    echo "input.sh: $*" >&2
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
    [ -n "$PYTHON" ] || die "không có python3 (cần để gửi lệnh monitor)"
    echo "# qemu $version ($QEMU)"
    echo "# opensbi $SBI"
    echo "# python $PYTHON"
}

# ---------------------------------------------------------------- build
build_image() {
    command -v cargo >/dev/null 2>&1 || die "không có cargo"
    if [ "${KT255_SKIP_BUILD:-0}" != "1" ]; then
        echo "# build ảnh input ($TARGET, release, --features ktest,input)"
        if ! cargo build --manifest-path "$KMANIFEST" -p vifast-kbin-riscv64 \
            --bin vifast-kbin-ktest-input --features ktest,input \
            --target "$TARGET" --release >"$LOGDIR/kt255-build-image.log" 2>&1; then
            tail -20 "$LOGDIR/kt255-build-image.log" >&2
            die "build ảnh input FAIL"
        fi
    fi
    [ -f "$IMAGE" ] || die "không thấy ảnh $IMAGE"
    # D2-72 Q5: trần `.data+.bss` 262.144 B — ảnh mới cũng chịu trần, không nới.
    if ! bash "$BUDGET" "$IMAGE" ktest-input >"$LOGDIR/kt255-budget.log" 2>&1; then
        cat "$LOGDIR/kt255-budget.log" >&2
        die "budget ảnh input VƯỢT trần"
    fi
    grep '^# image=' "$LOGDIR/kt255-budget.log" || true
}

# ---------------------------------------------------------------- bơm phím qua monitor
# `send_keys`: nối **một** phiên HMP, đọc banner tới `(qemu) `, gửi ba lệnh `sendkey`, đọc prompt
# sau mỗi lệnh. Trả 0 = đã gửi hết; 1 = không nối được / monitor không trả prompt.
send_keys() {
    "$PYTHON" - "$SOCK" <<'PY'
import socket
import sys
import time

sock = sys.argv[1]
commands = ["sendkey a", "sendkey shift-a", "sendkey f1"]
try:
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(5.0)
    s.connect(sock)
except OSError as exc:
    print(f"# input.sh: không nối được monitor {sock}: {exc}", file=sys.stderr)
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
for command in commands:
    s.sendall((command + "\n").encode())
    read_prompt()
    time.sleep(0.05)
s.close()
sys.exit(0)
PY
}

# `boot_keys <log>`: khởi động QEMU có bàn phím + monitor, chờ marker `# input: ready`, bơm ba lô
# phím, rồi chờ QEMU tự thoát trong ngân sách.
# Trả 0 = QEMU tự thoát; 124 = quá hạn (đã kill cả group); 125 = không khởi động được.
boot_keys() {
    local out="$1"
    rm -f "$SOCK" 2>/dev/null
    qg_tmp "$SOCK"
    qg_start "$out" "$TIMEOUT_DS" "$QEMU" -machine virt,aia=none,aclint=on -bios "$SBI" \
        -kernel "$IMAGE" -nographic -cpu rv64 -m 128M -smp 1 -no-reboot \
        -device virtio-keyboard-pci \
        -monitor "unix:$SOCK,server=on,wait=off" || return 125
    local pid="$QG_LAST_PID" waited="${QG_START_SPENT:-0}" i=0
    # 1) chờ marker của **khách** (không sleep mù): ảnh in sau khi đã arm 8 đệm.
    while [ "$i" -lt "$READY_DS" ] && kill -0 "$pid" 2>/dev/null; do
        grep -aq '^# input: ready$' "$out" 2>/dev/null && break
        sleep 0.1
        i=$((i + 1))
    done
    if grep -aq '^# input: ready$' "$out" 2>/dev/null; then
        if ! send_keys; then
            bad "ca dương: không gửi được sendkey qua monitor $SOCK"
        fi
    else
        bad "ca dương: không thấy marker '# input: ready' sau $((READY_DS / 10)) s"
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

# `validate_positive <log>`: 0 = log đạt **mọi** điều kiện của ca dương.
validate_positive() {
    local before="$FAILS"
    LOG="$1"
    if [ ! -f "$LOG" ]; then
        bad "ca dương: không có log $LOG"
        return 1
    fi
    # 1) TAP version + kế hoạch, đúng một lần.
    check_count "ca dương: 'TAP version 13'" '^TAP version 13$' 1
    check_count "ca dương: dòng kế hoạch '1..$TAP_COUNT'" "^1\\.\\.$TAP_COUNT\$" 1
    # 2) Năm dòng `ok N - <tên>` đúng thứ tự, đúng tên, mỗi dòng đúng một lần.
    check_tap 1 "input device"
    check_tap 2 "input name"
    check_tap 3 "input key a"
    check_tap 4 "input shift"
    check_tap 5 "input unknown key"
    # 3) Không `not ok` nào.
    check_count "ca dương: dòng 'not ok'" '^not ok' 0
    # 4) Đúng **một** dòng PANIC kết thúc, đúng mẫu.
    check_count "ca dương: dòng PANIC kết thúc" "$PANIC_OK" 1
    # 5) PCI: vendor/device **đọc từ thiết bị** + BAR gán đúng leaf 3 + đọc lại khớp.
    check_grep "vendor/device thật 'vendor=0x1af4 device=0x1012|0x1052' trong '# input pci:'" \
        "^# input pci: .*vendor=0x1af4 device=0x(1012|1052)( |\$)"
    check_grep "BAR gán/đọc lại '$BAR_ASSIGNED' trong '# input pci:'" \
        "^# input pci: .*assigned=$BAR_ASSIGNED readback=$BAR_ASSIGNED( |\$)"
    # 6) Tên **đọc từ** `virtio_input_config` (không hardcode trong ảnh).
    check_count "ca dương: dòng '# input name: QEMU Virtio Keyboard'" "$NAME_LINE" 1
    # 7) Feature/queue/bitmap: chỉ VERSION_1, **không** REP, thiết bị **có** phím 'a'.
    check_grep "'# input dev:' có name_len=20 version1=1 rep_feature=0 key_a_bit=1" \
        '^# input dev: .*name_len=20 .*version1=1 rep_feature=0 .*key_a_bit=1( |$)'
    check_grep "'# input dev:' có device_status=0xf (DRIVER_OK còn sau khi đọc config)" \
        '^# input dev: .*device_status=0xf( |$)'
    # 8) DMA identity + đã arm 8 đệm (điều kiện để QEMU không huỷ lô `sendkey shift-a`).
    check_grep "'# input dma:' có identity=1 armed=8" \
        '^# input dma: .*identity=1 armed=8( |$)'
    # 9) Marker bơm phím: đúng một lần (runner bơm **sau** khi khách arm đệm).
    check_count "ca dương: dòng '# input: ready'" '^# input: ready$' 1
    # 10) Ba lô phím: 'a' (0x61), Shift+'a' (0x41), phím không ASCII (none).
    check_grep "dòng '# input: key a code=30 ascii=0x61 shift=0'" \
        '^# input: key a code=30 ascii=0x61 shift=0( |$)'
    check_grep "dòng '# input: shift code=30 ascii=0x41 shift=1'" \
        '^# input: shift code=30 ascii=0x41 shift=1( |$)'
    check_grep "dòng '# input: unknown code=59 ascii=none'" \
        '^# input: unknown code=59 ascii=none( |$)'
    # 11) Bộ đếm sạch: không lỗi, không timeout, không lệch used->id.
    check_grep "'# input stats:' có errors=0 timeouts=0 id_mismatch=0" \
        '^# input stats: .*errors=0 timeouts=0 id_mismatch=0( |$)'
    # 12) Ca âm **không** được lẫn vào log ca dương.
    check_count "ca dương: dòng 'input: no device'" 'input: no device' 0
    return $((FAILS - before))
}

# `validate_negative <log>`: 0 = ảnh **đỏ đúng cách** khi không có thiết bị bàn phím.
validate_negative() {
    local before="$FAILS"
    LOG="$1"
    if [ ! -f "$LOG" ]; then
        bad "ca âm: không có log $LOG"
        return 1
    fi
    # 1) Ảnh phải in đúng dòng fail-closed.
    check_grep "dòng fail-closed 'input: no device'" 'input: no device'
    # 2) TAP 1 phải **đỏ**.
    check_count "ca âm: 'not ok 1 - input device'" '^not ok 1 - input device' 1
    # 3) **Không** được có `ok 1` — đây là chỗ bắt "PASS giả".
    check_count "ca âm: 'ok 1 - input device'" '^ok 1 - input device' 0
    # 4) **Không** được có dòng kết thúc "xong" (nghĩa là đã đi hết 5 TAP).
    check_count "ca âm: dòng PANIC 'xong'" "$PANIC_OK" 0
    # 5) **Không** được có marker bơm phím (ảnh dừng trước khi tới đó).
    check_count "ca âm: dòng '# input: ready'" '^# input: ready$' 0
    return $((FAILS - before))
}

# ---------------------------------------------------------------- hai ca
run_positive() {
    local log="$LOGDIR/kt255-positive.log"
    echo "# ca dương: -device virtio-keyboard-pci + monitor sendkey a / shift-a / f1"
    if ! boot_keys "$log"; then
        bad "ca dương: QEMU không tự thoát trong 20 s (timeout/kill cả group)"
        return 1
    fi
    if validate_positive "$log"; then
        ok "ca dương: 5/5 TAP + tên thật + 'a'/'A'/mã lạ + bộ đếm sạch ($log)"
        return 0
    fi
    return 1
}

run_negative() {
    local log="$LOGDIR/kt255-negative.log"
    echo "# ca âm: cùng ảnh, KHÔNG -device virtio-keyboard-pci"
    if ! qg_boot "$log" "$TIMEOUT_DS" "$QEMU" -machine virt,aia=none,aclint=on -bios "$SBI" \
        -kernel "$IMAGE" -nographic -cpu rv64 -m 128M -smp 1 -no-reboot; then
        bad "ca âm: QEMU không tự thoát trong 20 s (timeout/kill cả group)"
        return 1
    fi
    if validate_negative "$log"; then
        ok "ca âm: ảnh in 'input: no device' + TAP 1 đỏ, không PASS giả ($log)"
        return 0
    fi
    return 1
}

# ---------------------------------------------------------------- tự kiểm validator
# `expect_accept <mô tả> <hàm> <log>` / `expect_reject …`: chạy validator và đòi kết quả.
# Lỗi do **ca cố ý xấu** không tính vào `FAILS` cuối (chỉ tính khi kỳ vọng sai).
expect_accept() {
    local what="$1" fn="$2" log="$3" before="$FAILS"
    if "$fn" "$log" >/dev/null 2>&1; then
        ok "selftest: $what được chấp nhận"
    else
        bad "selftest: $what bị TỪ CHỐI (validator quá chặt/sai)"
    fi
    FAILS="$before"
}

expect_reject() {
    local what="$1" fn="$2" log="$3" before="$FAILS"
    if "$fn" "$log" >/dev/null 2>&1; then
        bad "selftest: $what KHÔNG bị bắt (validator rỗng)"
    else
        ok "selftest: $what bị từ chối"
    fi
    FAILS="$before"
}

red_selftest() {
    local good="$LOGDIR/kt255-selftest-good.log"
    local neg="$LOGDIR/kt255-selftest-neg.log"
    local tmp="$LOGDIR/kt255-selftest-mut.log"
    # Log **tốt** dựng tay: đúng năm TAP + đủ bằng chứng (không phải log QEMU thật — chỉ để kiểm
    # validator, không dùng làm bằng chứng KT-255).
    cat >"$good" <<'EOF'
TAP version 13
1..5
# input image: hart=0 stvec=0x802000a0 direct=1 satp=0x80000000000804e8 mode=8 tables=10 used=40960 const:wait_ms=10000 const:event_bufs=8
# input pci: functions=2 bus=0 dev=1 func=0 vendor=0x1af4 device=0x1052 bar=4 kind=mem64 size=16384 assigned=0x40600000 readback=0x40600000 command=0x6
# input cap: common=0x0 notify=0x3000 device=0x2000 isr=0x1000 mul=4 cap_bar=4 ecam=0x30008000 ecam_identity=1 bar_identity=1 cap_ranges_ok=1 overlap_checked=1 other_bars=0 reserved_ranges=5
ok 1 - input device
# input name: QEMU Virtio Keyboard
# input dev: name_len=20 devids_bustype=6 devids_vendor=0x627 devids_product=0x1 devids_version=0x1 num_queues=2 queue_max=64 queue_size=16 device_features=0x10130000000 driver_features=0x100000000 version1=1 rep_feature=0 device_status=0xf config_generation=6 ev_bits_len=29 key_a_bit=1
# input dma: desc=0x80209000 avail=0x80209100 used=0x80209128 buf=0x80209400 identity=1 armed=8 notify_off=0 notify_addr=0x40603000 queue=0
ok 2 - input name
# input: ready
# input: key a code=30 ascii=0x61 shift=0 elapsed_ms=897
ok 3 - input key a
# input: shift code=30 ascii=0x41 shift=1 elapsed_ms=897
ok 4 - input shift
# input: unknown code=59 ascii=none const:f1=59 elapsed_ms=897
ok 5 - input unknown key
# input stats: events=13 syncs=6 key_downs=4 key_ups=3 notifies=14 errors=0 timeouts=0 id_mismatch=0 published=21 queue_size=16 device_status=0xf
PANIC: KT-255 input xong (5 TAP, 0 fail)
EOF
    expect_accept "log tốt dựng tay" validate_positive "$good"
    # Mười một đột biến phải bị **từ chối** — nếu không, validator rỗng.
    local name
    for name in xoa_ok3 doi_ascii doi_shift_ascii doi_unknown doi_name doi_vendor doi_armed \
        doi_identity them_not_ok xoa_stats xoa_ready; do
        case "$name" in
            xoa_ok3) grep -v '^ok 3 - input key a$' "$good" >"$tmp" ;;
            doi_ascii) sed 's/ascii=0x61/ascii=0x62/' "$good" >"$tmp" ;;
            doi_shift_ascii) sed 's/ascii=0x41/ascii=0x61/' "$good" >"$tmp" ;;
            doi_unknown) sed 's/ascii=none/ascii=0x00/' "$good" >"$tmp" ;;
            doi_name) sed 's/^# input name: QEMU Virtio Keyboard$/# input name: Generic Keyboard/' \
                "$good" >"$tmp" ;;
            doi_vendor) sed 's/vendor=0x1af4/vendor=0x1234/' "$good" >"$tmp" ;;
            doi_armed) sed 's/armed=8/armed=1/' "$good" >"$tmp" ;;
            doi_identity) sed 's/identity=1/identity=0/' "$good" >"$tmp" ;;
            them_not_ok) sed 's/^ok 2 - input name$/not ok 2 - input name/' "$good" >"$tmp" ;;
            xoa_stats) grep -v '^# input stats:' "$good" >"$tmp" ;;
            xoa_ready) grep -v '^# input: ready$' "$good" >"$tmp" ;;
        esac
        expect_reject "đột biến '$name'" validate_positive "$tmp"
    done
    # Ca âm "không thiết bị": log đỏ đúng cách ⇒ chấp nhận; log "PASS giả" (có `ok 1`) ⇒ từ chối.
    cat >"$neg" <<'EOF'
TAP version 13
1..5
# input pci: err=NoDevice stage=nodevice
# input: no device
not ok 1 - input device: không tìm/khởi tạo được PCI
# ktest halt
EOF
    expect_accept "log ca âm 'không thiết bị' đỏ đúng cách" validate_negative "$neg"
    sed 's/^not ok 1 - input device.*$/ok 1 - input device/' "$neg" >"$tmp"
    expect_reject "ca âm 'không thiết bị' PASS giả (có 'ok 1')" validate_negative "$tmp"
    # Ca âm "PASS giả" thứ hai: có `ok 1` **và** dòng PANIC "xong" ⇒ vẫn phải bị từ chối.
    {
        cat "$neg"
        echo "PANIC: KT-255 input xong (5 TAP, 0 fail)"
    } >"$tmp"
    expect_reject "ca âm có dòng PANIC 'xong'" validate_negative "$tmp"
    rm -f "$tmp" "$good" "$neg"
}

# ---------------------------------------------------------------- main
MODE=run
CHECK_LOG=""
CHECK_KIND=positive
case "${1:-}" in
    --red-selftest) MODE=selftest ;;
    --check-log)
        MODE=check
        CHECK_LOG="${2:-}"
        CHECK_KIND="${3:-positive}"
        [ -n "$CHECK_LOG" ] || die "--check-log cần đường dẫn log"
        ;;
    "" | --run) MODE=run ;;
    -h | --help)
        sed -n '2,51p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    *) die "tham số không biết: $1" ;;
esac

if [ "$MODE" = selftest ]; then
    red_selftest
    echo "# input.sh --red-selftest: $CHECKS kiểm, $FAILS lỗi"
    [ "$FAILS" = 0 ] || exit 1
    exit 0
fi

if [ "$MODE" = check ]; then
    case "$CHECK_KIND" in
        positive) validate_positive "$CHECK_LOG" ;;
        negative) validate_negative "$CHECK_LOG" ;;
        *) die "--check-log: kiểu phải là positive|negative" ;;
    esac
    rc=$?
    echo "# input.sh --check-log $CHECK_KIND $CHECK_LOG: rc=$rc"
    exit "$rc"
fi

find_qemu
build_image
QG_LOG="$LOGDIR/qemu-guard.log"
rm -f "$QG_LOG" 2>/dev/null # log guard **mới** mỗi lần chạy (tổng hợp theo run)
qg_init || die "qemu_guard: không khởi tạo được (thiếu perl?) — OD-K6-6"

run_positive || true
run_negative || true
qg_infra "kt255-input" "$LOGDIR/kt255-infra.log" || true

echo "# input.sh: $CHECKS kiểm đạt, $FAILS lỗi (log: $LOGDIR)"
if [ "$FAILS" != 0 ]; then
    exit 1
fi
exit 0
