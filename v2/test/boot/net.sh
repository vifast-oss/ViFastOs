#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-245 (D2-196) — runner ảnh **test virtio-net** riscv64 (`vifast-kbin-ktest-net`).
#
# Vì sao runner riêng (không mở rộng `ktest.sh`): ảnh này cần **thiết bị mạng** trong dòng QEMU
# (`-netdev user,id=n0 -device virtio-net-pci,netdev=n0`) và có **ca âm** riêng (cùng ảnh, **không**
# có `-netdev`/`-device` ⇒ phải in `net: no device` + TAP đỏ). Nhét vào `ktest.sh` sẽ phải sửa
# manifest/expected.txt của 21 boot matrix cũ — rủi ro xung đột với các nhánh khác (KT-243..KT-251).
#
# Hợp đồng (fail-closed, không nới ngưỡng):
#   1. **Ca dương**: boot ảnh với `-netdev user` + `-device virtio-net-pci`; log phải có
#      `TAP version 13` + `1..4` đúng một lần, bốn dòng `ok 1..4` đúng **thứ tự** và đúng **tên**,
#      **không** `not ok`, đúng **một** dòng `PANIC: KT-245 net xong (4 TAP, 0 fail)`, và các dòng
#      bằng chứng: MAC thật `52:54:00:…` (OUI QEMU), `# net rx:` có `oper=2` + `sender_ip=10.0.2.2`,
#      `# net stats:` có `errors=0 timeouts=0 id_mismatch=0`.
#   2. **Ca âm**: **cùng** ảnh, **không** `-netdev`/`-device` ⇒ log phải có `net: no device` +
#      `not ok 1 - net device`, **không** có `ok 1`, **không** có dòng PANIC "xong". Runner coi ca
#      âm **đạt** khi ảnh đỏ đúng cách; nếu ảnh PASS giả ⇒ runner **đỏ**.
#   3. `--red-selftest`: chứng minh bộ kiểm **không rỗng** — sửa log tốt (xoá `ok 3`, đổi MAC,
#      đổi `oper`, thêm `not ok`) rồi đòi validator **từ chối** từng bản; và một log ca âm "PASS
#      giả" (có `ok 1`) phải bị từ chối.
#
# Dùng:
#   bash v2/test/boot/net.sh                 # build + ca dương + ca âm (PASS/FAIL)
#   bash v2/test/boot/net.sh --red-selftest  # chỉ tự kiểm validator (không build, không QEMU)
#   bash v2/test/boot/net.sh --check-log <log> [positive|negative]   # kiểm MỘT log đã có
#   KT245_SKIP_BUILD=1 bash v2/test/boot/net.sh                      # dùng ảnh có sẵn
#   KT245_LOGDIR=<dir> bash v2/test/boot/net.sh                      # nơi để log
# Exit: 0 = ca dương xanh + ca âm đỏ đúng; 1 = có lỗi; 2 = lỗi cấu hình môi trường.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
TARGET="riscv64gc-unknown-none-elf"
KMANIFEST="$V2_ROOT/kernel/Cargo.toml"
IMAGE="${KT245_IMAGE:-$V2_ROOT/kernel/target/$TARGET/release/vifast-kbin-ktest-net}"
LOGDIR="${KT245_LOGDIR:-${V2_RUN_TMP:-${TMPDIR:-/tmp}}/kt245-net}"
BUDGET="$HERE/budget.sh"
# KT-242 (P3): LOGDIR riêng theo lần chạy ⇒ tạo trước lần ghi log đầu tiên.
mkdir -p "$LOGDIR" 2>/dev/null || true
TIMEOUT_DS=200 # 20 s/boot (D2-72 Q10), đơn vị 0,1 s
QEMU_ICOUNT="-icount shift=6" # D2-74 (chỉ runner; run.sh/soak không icount)
QEMU_MIN_MAJOR=9
QEMU_MIN_MINOR=2
# Số TAP của ảnh — runner đòi đúng bốn dòng `ok 1..4` đúng **tên** (thêm/bớt TAP phải sửa cả
# kernel, tệp này và `# net stats` trong cùng commit).
TAP_COUNT=4
# Dòng kết thúc chủ đích của ảnh (khớp `ktest_net.rs`).
PANIC_OK='^PANIC: KT-245 net xong \(4 TAP, 0 fail\)$'
# OUI mà QEMU gán cho card slirp — MAC **đọc từ thiết bị**, hằng này chỉ để đối chiếu nguồn gốc.
MAC_PREFIX="52:54:00:"

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

FAILS=0
CHECKS=0

die() {
    echo "net.sh: $*" >&2
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
    echo "# qemu $version ($QEMU)"
    echo "# opensbi $SBI"
}

# ---------------------------------------------------------------- build
build_image() {
    command -v cargo >/dev/null 2>&1 || die "không có cargo"
    if [ "${KT245_SKIP_BUILD:-0}" != "1" ]; then
        echo "# build ảnh net ($TARGET, release, --features ktest,net)"
        if ! cargo build --manifest-path "$KMANIFEST" -p vifast-kbin-riscv64 \
            --bin vifast-kbin-ktest-net --features ktest,net \
            --target "$TARGET" --release >"$LOGDIR/kt245-build-image.log" 2>&1; then
            tail -20 "$LOGDIR/kt245-build-image.log" >&2
            die "build ảnh net FAIL"
        fi
    fi
    [ -f "$IMAGE" ] || die "không thấy ảnh $IMAGE"
    # D2-72 Q5: trần `.data+.bss` 262.144 B — ảnh mới cũng chịu trần, không nới.
    if ! bash "$BUDGET" "$IMAGE" ktest-net >"$LOGDIR/kt245-budget.log" 2>&1; then
        cat "$LOGDIR/kt245-budget.log" >&2
        die "budget ảnh net VƯỢT trần"
    fi
    grep '^# image=' "$LOGDIR/kt245-budget.log" || true
}

# ---------------------------------------------------------------- boot
# `boot <log> [tham số QEMU thêm]`: 0 = QEMU tự thoát; 124 = quá 20 s đã kill cả group.
boot() {
    local out="$1"
    shift
    qg_boot "$out" "$TIMEOUT_DS" "$QEMU" $QEMU_ICOUNT -machine virt,aia=none,aclint=on -bios "$SBI" \
        -kernel "$IMAGE" -nographic -cpu rv64 -m 128M -smp 1 -no-reboot "$@"
}

# ---------------------------------------------------------------- validator
# Mọi phép kiểm đi qua `bad` (tăng `FAILS` **toàn cục**) và validator trả **hiệu số** ⇒ không có
# đường "kiểm đỏ mà vẫn trả 0" (bẫy đã tự gặp ở bản đầu: `bad` tăng biến toàn cục còn validator
# trả biến cục bộ ⇒ log xấu vẫn được chấp nhận).
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
    check_count "ca dương: dòng kế hoạch '1..$TAP_COUNT'" "^1\.\.$TAP_COUNT\$" 1
    # 2) Bốn dòng `ok N - <tên>` đúng thứ tự, đúng tên, mỗi dòng đúng một lần.
    check_tap 1 "net device"
    check_tap 2 "net mac"
    check_tap 3 "net tx"
    check_tap 4 "net rx"
    # 3) Không `not ok` nào.
    check_count "ca dương: dòng 'not ok'" '^not ok' 0
    # 4) Đúng **một** dòng PANIC kết thúc, đúng mẫu.
    check_count "ca dương: dòng PANIC kết thúc" "$PANIC_OK" 1
    # 5) Bằng chứng MAC **thật** của QEMU (đọc từ thiết bị, không hardcode trong ảnh).
    check_grep "MAC thật '${MAC_PREFIX}…' trong '# net dev:'" \
        "^# net dev: mac=${MAC_PREFIX}[0-9a-f]{2}:[0-9a-f]{2}:[0-9a-f]{2}( |\$)"
    # 6) Bằng chứng RX: ARP reply của slirp (oper=2, sender 10.0.2.2, target MAC khớp).
    check_grep "ARP reply 'oper=2 sender_ip=10.0.2.2' trong '# net rx:'" \
        '^# net rx: frame=[0-9]+ oper=2 ethertype=0x806 sender_ip=10\.0\.2\.2( |$)'
    check_grep "ARP reply trỏ đúng MAC của ta (target_mac_khop=1)" \
        '^# net rx: .*target_mac_khop=1( |$)'
    # 7) Bộ đếm sạch: không lỗi, không timeout, không lệch used->id.
    check_grep "'# net stats:' có errors=0 timeouts=0 id_mismatch=0" \
        '^# net stats: .*errors=0 timeouts=0 id_mismatch=0( |$)'
    # 8) Ca âm **không** được lẫn vào log ca dương.
    check_count "ca dương: dòng 'net: no device'" 'net: no device' 0
    return $((FAILS - before))
}

# `validate_negative <log>`: 0 = ảnh **đỏ đúng cách** khi không có thiết bị mạng.
validate_negative() {
    local before="$FAILS"
    LOG="$1"
    if [ ! -f "$LOG" ]; then
        bad "ca âm: không có log $LOG"
        return 1
    fi
    # 1) Ảnh phải in đúng dòng fail-closed.
    check_grep "dòng fail-closed 'net: no device'" 'net: no device'
    # 2) TAP 1 phải **đỏ**.
    check_count "ca âm: 'not ok 1 - net device'" '^not ok 1 - net device' 1
    # 3) **Không** được có `ok 1` — đây là chỗ bắt "PASS giả".
    check_count "ca âm: 'ok 1 - net device'" '^ok 1 - net device' 0
    # 4) **Không** được có dòng kết thúc "xong" (nghĩa là đã đi hết 4 TAP).
    check_count "ca âm: dòng PANIC 'xong'" "$PANIC_OK" 0
    return $((FAILS - before))
}

# ---------------------------------------------------------------- hai ca
run_positive() {
    local log="$LOGDIR/kt245-positive.log"
    echo "# ca dương: -netdev user,id=n0 -device virtio-net-pci,netdev=n0"
    if ! boot "$log" -netdev user,id=n0 -device virtio-net-pci,netdev=n0; then
        bad "ca dương: QEMU không tự thoát trong 20 s (timeout/kill cả group)"
        return 1
    fi
    if validate_positive "$log"; then
        ok "ca dương: 4/4 TAP + MAC thật + ARP reply + bộ đếm sạch ($log)"
        return 0
    fi
    return 1
}

run_negative() {
    local log="$LOGDIR/kt245-negative.log"
    echo "# ca âm: cùng ảnh, KHÔNG -netdev/-device"
    if ! boot "$log"; then
        bad "ca âm: QEMU không tự thoát trong 20 s (timeout/kill cả group)"
        return 1
    fi
    if validate_negative "$log"; then
        ok "ca âm: ảnh in 'net: no device' + TAP 1 đỏ, không PASS giả ($log)"
        return 0
    fi
    return 1
}

# ---------------------------------------------------------------- tự kiểm validator
# `expect_accept <mô tả> <hàm> <log>` / `expect_reject …`: chạy validator và đòi kết quả.
# Lỗi do **ca cố ý xấu** không tính vào `FAILS` cuối (chỉ tính khi kỳ vọng sai) — nếu không, dòng
# tổng kết sẽ báo "N lỗi" dù validator đúng.
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
    local good="$LOGDIR/kt245-selftest-good.log"
    local neg="$LOGDIR/kt245-selftest-neg.log"
    local tmp="$LOGDIR/kt245-selftest-mut.log"
    # Log **tốt** dựng tay: đúng bốn TAP + đủ bằng chứng (không phải log QEMU thật — chỉ để kiểm
    # validator, không dùng làm bằng chứng KT-245).
    cat >"$good" <<'EOF'
TAP version 13
1..4
# net dev: mac=52:54:00:12:34:56 status=0x1 mtu_raw=65535
ok 1 - net device
ok 2 - net mac
# net tx: frame=42 (=14 ethernet + 28 arp) result=ok
ok 3 - net tx
# net rx: frame=64 oper=2 ethertype=0x806 sender_ip=10.0.2.2 sender_mac=52:55:0a:00:02:02 target_mac_khop=1
ok 4 - net rx
# net stats: tx_frames=1 tx_bytes=42 rx_frames=1 rx_bytes=64 notifies=3 errors=0 timeouts=0 id_mismatch=0
PANIC: KT-245 net xong (4 TAP, 0 fail)
EOF
    expect_accept "log tốt dựng tay" validate_positive "$good"
    # Năm đột biến phải bị **từ chối** — nếu không, validator rỗng.
    local name
    for name in xoa_ok3 doi_mac doi_oper them_not_ok xoa_stats; do
        case "$name" in
            xoa_ok3) grep -v '^ok 3 - net tx$' "$good" >"$tmp" ;;
            doi_mac) sed 's/mac=52:54:00:/mac=02:00:00:/' "$good" >"$tmp" ;;
            doi_oper) sed 's/oper=2/oper=1/' "$good" >"$tmp" ;;
            them_not_ok) sed 's/^ok 2 - net mac$/not ok 2 - net mac/' "$good" >"$tmp" ;;
            xoa_stats) grep -v '^# net stats:' "$good" >"$tmp" ;;
        esac
        expect_reject "đột biến '$name'" validate_positive "$tmp"
    done
    # Ca âm: log đỏ đúng cách ⇒ chấp nhận; log "PASS giả" (có `ok 1`) ⇒ từ chối.
    cat >"$neg" <<'EOF'
TAP version 13
1..4
# net pci: err=NoDevice stage=nodevice
# net: no device
not ok 1 - net device: không tìm/khởi tạo được PCI
# ktest halt
EOF
    expect_accept "log ca âm đỏ đúng cách" validate_negative "$neg"
    sed 's/^not ok 1 - net device.*$/ok 1 - net device/' "$neg" >"$tmp"
    expect_reject "ca âm 'PASS giả' (có 'ok 1')" validate_negative "$tmp"
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
        sed -n '2,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    *) die "tham số không biết: $1" ;;
esac

if [ "$MODE" = selftest ]; then
    red_selftest
    echo "# net.sh --red-selftest: $CHECKS kiểm, $FAILS lỗi"
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
    echo "# net.sh --check-log $CHECK_KIND $CHECK_LOG: rc=$rc"
    exit "$rc"
fi

find_qemu
build_image
QG_DIR="${TMPDIR:-/tmp}"
QG_LOG="$LOGDIR/qemu-guard.log"
rm -f "$QG_LOG" 2>/dev/null # log guard **mới** mỗi lần chạy (tổng hợp theo run)
qg_init || die "qemu_guard: không khởi tạo được (thiếu perl?) — OD-K6-6"

run_positive || true
run_negative || true
qg_infra "kt245-net" "$LOGDIR/kt245-infra.log" || true

echo "# net.sh: $CHECKS kiểm đạt, $FAILS lỗi (log: $LOGDIR)"
if [ "$FAILS" != 0 ]; then
    exit 1
fi
exit 0
