#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-406** — runner ảnh **SẢN PHẨM x86_64** lái **shell thật** qua **COM1 RX IRQ thật**
# (`IRQ 4 → IOAPIC → LAPIC → IDT 0x24 → ring KT-69 → shell`) và chạy **kịch bản chung 3 arch**
# (`lib/product-scenario.sh`): `help` → `echo KT406` → `ps` → `exit`.
#
# # Vì sao có runner riêng
#
# `run-x86_64.sh` (T1) boot ảnh product-shell với **stdin `/dev/null`** và đòi script kernel bơm
# vào ring (`--features product-shell,script-input`) — đường **kiểm hồi quy** của đường cũ. Runner
# này boot ảnh **sản phẩm thật** (không `script-input`): ring khởi tạo **rỗng**, mọi byte tới shell
# phải qua COM1 và **IRQ 4** thật. Byte được gõ từ **stdin QEMU** (FIFO).
#
# # Việc runner làm (fail-closed)
#
#   1. build `--features product-shell` (mặc định; **không** `script-input`);
#   2. `budget-x86_64.sh <ảnh> x86_64-product-shell` (trần 256 KiB, D2-250);
#   3. quét **0** harness marker (`lib/harness-scan.sh`, 23 tiền tố D2-08);
#   4. boot QEMU `-M q35 -smp 1`, stdin = FIFO; chờ `shell>` của **khách** rồi gõ từng lệnh,
#      chờ **evidence của khách** trước khi gõ lệnh kế (không `sleep` mù);
#   5. kiểm: `# uart: … ok=1` + `# uart: irq≥1 rx≥1 drops=0 … ok=1` + kịch bản chung + marker
#      đúng 1 lần + halt + **0** TAP;
#   6. **ca âm** (`--negative minimal`): ảnh **tối giản** boot bình thường nhưng **không** có
#      `shell>`/`# uart:`.
#
# Dùng:
#   bash v2/test/boot/product-x86_64.sh [--image <ảnh>] [--features <ds>]
#                                       [--negative minimal] [--print-logdir] [--help]
#
# Exit: 0 = mọi phép kiểm/đúng ca âm; 1 = có phép kiểm đỏ; 2 = lỗi cấu hình/môi trường;
#       3 = TỪ CHỐI ĐO (máy đo đang bận — D2-245/D2-248).
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=/dev/null
. "$HERE/lib/env.sh"
# shellcheck source=/dev/null
. "$HERE/lib/harness-scan.sh"
# shellcheck source=/dev/null
. "$HERE/lib/product-scenario.sh"
# shellcheck source=/dev/null
. "$HERE/build-rev.sh"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
BUDGET="$HERE/budget-x86_64.sh"
TARGET="x86_64-unknown-none"
# Tôn trọng `CARGO_TARGET_DIR` (cùng quy ước cargo): worktree dùng chung target dir của slot thì
# ảnh nằm ở `$CARGO_TARGET_DIR/<target>/release`; không đặt thì như các runner cũ.
IMG_DIR="${CARGO_TARGET_DIR:-$V2_ROOT/kernel/target}/$TARGET/release"
IMAGE=""
FEATURES="product-shell"
NEG=""
X64_CPU="${V2_X64_CPU:-qemu64}"
TIMEOUT_DS=400
READY_DS=200
LOGDIR="$(v2_env_logdir kt406-product-x86_64 KT406_X86_64_LOGDIR)"

CHECKS=0
FAILS=0

die() {
    echo "product-x86_64.sh: $*" >&2
    exit 2
}
good() {
    CHECKS=$((CHECKS + 1))
    echo "ok $CHECKS - $*"
}
bad() {
    CHECKS=$((CHECKS + 1))
    FAILS=$((FAILS + 1))
    echo "not ok $CHECKS - $*"
}

usage() {
    cat <<'EOF'
Dùng: bash v2/test/boot/product-x86_64.sh [--image <ảnh>] [--features <ds>]
                                         [--negative minimal] [--print-logdir] [--help]
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --image) IMAGE="${2:-}"; shift 2 ;;
        --features) FEATURES="${2:-}"; shift 2 ;;
        --negative) NEG="${2:-}"; shift 2 ;;
        --print-logdir) echo "$LOGDIR"; exit 0 ;;
        -h | --help) usage; exit 0 ;;
        *) die "tham số lạ: $1" ;;
    esac
done
case "$NEG" in '' | minimal) ;; *) die "--negative phải là 'minimal' (hoặc bỏ trống)" ;; esac

[ -f "$MANIFEST" ] || die "không thấy $MANIFEST"
[ -f "$BUDGET" ] || die "không thấy $BUDGET"

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"
mkdir -p "$LOGDIR" || die "không tạo được logdir $LOGDIR"
QEMU="$(command -v qemu-system-x86_64 || true)"
[ -n "$QEMU" ] || die "không có qemu-system-x86_64"

# shellcheck source=/dev/null
. "$V2_ROOT/tools/measure-lock.sh"
if measure_lock_is_held; then
    echo "# product-x86_64.sh: TỪ CHỐI ĐO — máy đo đang giữ khoá (T2/soak của Lead)" >&2
    exit 3
fi

build_image() { # $1 = danh sách feature (rỗng = ảnh tối giản), $2 = log
    local feats="$1"
    local args=(--manifest-path "$MANIFEST" -p vifast-kbin-x86_64 --target "$TARGET" --release)
    [ -n "$feats" ] && args+=(--features "$feats")
    if ! cargo build "${args[@]}" >"$2" 2>&1; then
        tail -20 "$2" >&2
        return 1
    fi
    return 0
}

has() { LC_ALL=C grep -a -q -E "$2" "$1" 2>/dev/null; }
rev_line() { LC_ALL=C grep -am1 '^# build: ' "$1" 2>/dev/null || true; }

# ---------------------------------------------------------------- 1) ảnh sản phẩm-shell
echo "# build ảnh SẢN PHẨM-SHELL x86_64 ($TARGET, release, features=$FEATURES)"
build_image "$FEATURES" "$LOGDIR/build-product-shell.log" ||
    die "build ảnh product-shell x86_64 FAIL (xem $LOGDIR/build-product-shell.log)"
[ -z "$IMAGE" ] && IMAGE="$IMG_DIR/vifast-kbin-x86_64"
[ -x "$IMAGE" ] || die "không thấy ảnh $IMAGE"
cp "$IMAGE" "$LOGDIR/vifast-kbin-x86_64-product-shell" ||
    die "không copy được ảnh product-shell"
PS_IMAGE="$LOGDIR/vifast-kbin-x86_64-product-shell"

if bash "$BUDGET" "$PS_IMAGE" "x86_64-product-shell" >"$LOGDIR/budget.log" 2>&1; then
    good "budget: $(grep -E '^# image=' "$LOGDIR/budget.log" | head -1)"
else
    bad "budget: ảnh product-shell VƯỢT trần (xem $LOGDIR/budget.log)"
    sed -n '1,3p' "$LOGDIR/budget.log" >&2
fi

HITS="$(harness_scan_count_strings "$PS_IMAGE")" || die "không đọc được ảnh (TOCTOU)"
if [ "${HITS:-1}" -eq 0 ]; then
    good "D2-08: 0 harness marker (23 tiền tố) trong ảnh sản phẩm"
else
    bad "D2-08: ${HITS:-?} harness marker trong ảnh sản phẩm"
fi

# ---------------------------------------------------------------- 2) boot + gõ kịch bản qua UART
send_wait() { # <fd> <log> <pid> <cmd> <regex>
    local fd="$1" log="$2" pid="$3" cmd="$4" re="$5" n=0
    printf '%s\n' "$cmd" >&"$fd"
    while [ "$n" -lt 200 ]; do
        has "$log" "$re" && return 0
        kill -0 "$pid" 2>/dev/null || return 1
        sleep 0.05
        n=$((n + 1))
    done
    return 1
}

boot_uart_scenario() { # <log> <image>
    local log="$1" image="$2" fifo pid waited rc=0
    fifo="$LOGDIR/fifo.$$"
    rm -f "$fifo"
    mkfifo "$fifo" || die "không tạo được FIFO"
    : >"$log"
    "$QEMU" -M q35 -cpu "$X64_CPU" -m 128M -nographic -no-reboot -smp 1 -nic none \
        -kernel "$image" <"$fifo" >"$log" 2>&1 &
    pid=$!
    exec 8<>"$fifo"
    waited=0
    while [ "$waited" -lt "$READY_DS" ] && ! has "$log" '^shell>$'; do
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.1
        waited=$((waited + 1))
    done
    if ! has "$log" '^shell>$'; then
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
        exec 8>&-
        rm -f "$fifo"
        return 1
    fi
    local cmd re
    while IFS=$'\t' read -r cmd re; do
        if ! send_wait 8 "$log" "$pid" "$cmd" "$re"; then
            echo "# thiếu evidence cho '$cmd' (/$re/) — log $log" >&2
            rc=1
            break
        fi
    done < <(product_scenario_lines)
    waited=0
    while [ "$waited" -lt "$TIMEOUT_DS" ]; do
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.1
        waited=$((waited + 1))
    done
    if kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null || true
        rc=124
    fi
    wait "$pid" 2>/dev/null
    local st=$?
    [ "$st" -eq 0 ] || [ "$rc" -ne 0 ] || rc="$st"
    exec 8>&-
    rm -f "$fifo"
    return "$rc"
}

PS_LOG="$LOGDIR/product-shell-boot.log"
if ! boot_uart_scenario "$PS_LOG" "$PS_IMAGE"; then
    bad "boot + kịch bản UART: QEMU không hoàn tất sạch (log $PS_LOG)"
else
    good "boot: ảnh sản phẩm-shell chạy hết kịch bản chung qua UART stdin"
fi

scenario_out="$(product_scenario_check "$PS_LOG")"
if [ -z "$scenario_out" ]; then
    good "kịch bản chung 3 arch: help/echo/ps/exit đủ evidence"
else
    bad "kịch bản chung: $(printf '%s' "$scenario_out" | tr '\n' ' ')"
fi

uart_out="$(product_uart_irq_check "$PS_LOG")"
if [ -z "$uart_out" ]; then
    good "uart: COM1 RX IRQ thật — $(LC_ALL=C grep -aE '^# uart: irq=[0-9]+ ' "$PS_LOG" | tail -1)"
else
    bad "uart: $(printf '%s' "$uart_out" | tr '\n' ' ')"
fi

if has "$PS_LOG" '^# core: .* in_image=1 .*healthy=1$'; then
    good "core: Machine trong ảnh (in_image=1 healthy=1)"
else
    bad "core: thiếu '# core: … in_image=1 … healthy=1'"
fi
N_MARK="$(LC_ALL=C grep -acx '# viFastOs x86_64: boot ok' "$PS_LOG" 2>/dev/null || true)"
[ "${N_MARK:-0}" -eq 1 ] && good "marker đúng 1 lần" || bad "marker ${N_MARK:-0} lần (cần đúng 1)"
if LC_ALL=C grep -aqx '# viFastOs x86_64: halt' "$PS_LOG"; then
    good "halt sạch (ACPI shutdown)"
else
    bad "thiếu dòng '# viFastOs x86_64: halt'"
fi
if LC_ALL=C grep -aEq '^(TAP version 13|not ok|ok [0-9]+ - |1\.\.[0-9]+)' "$PS_LOG"; then
    bad "log sản phẩm chứa dấu vết harness (D2-08)"
else
    good "D2-08: 0 dấu vết harness trong log"
fi
if has "$PS_LOG" '^# build: algo=kt134-v5 '; then
    good "build identity: $(rev_line "$PS_LOG")"
else
    bad "thiếu dòng '# build: algo=…' của ảnh"
fi
# D2-80: dòng `# conf image:` do **kernel** in phải khớp hash ELF của **đúng** ảnh vừa boot.
PY="$(command -v python3.12 || command -v python3 || true)"
if [ -n "$PY" ]; then
    DIGEST_LINE="$("$PY" "$V2_ROOT/test/conformance/image_digest.py" "$PS_IMAGE" 2>/dev/null || true)"
    if [ -n "$DIGEST_LINE" ] && LC_ALL=C grep -aqx "# conf image: $DIGEST_LINE" "$PS_LOG"; then
        good "D2-80: '# conf image:' khớp hash ELF của ảnh vừa boot"
    else
        bad "D2-80: '# conf image:' không khớp ELF (cần '$DIGEST_LINE')"
    fi
fi

# ---------------------------------------------------------------- 3) ca âm: ảnh tối giản KHÔNG có shell
if [ "$NEG" = "minimal" ]; then
    MIN_IMAGE="$LOGDIR/vifast-kbin-x86_64-minimal"
    if ! build_image "" "$LOGDIR/build-minimal.log"; then
        bad "build ảnh tối giản FAIL (xem $LOGDIR/build-minimal.log)"
    else
        cp "$IMG_DIR/vifast-kbin-x86_64" "$MIN_IMAGE" || die "không copy được ảnh tối giản"
        MIN_LOG="$LOGDIR/minimal-boot.log"
        if "$QEMU" -M q35 -cpu "$X64_CPU" -m 128M -nographic -no-reboot -smp 1 -nic none \
            -kernel "$MIN_IMAGE" </dev/null >"$MIN_LOG" 2>&1; then
            if LC_ALL=C grep -aq '^shell>$' "$MIN_LOG" || LC_ALL=C grep -aq '^# uart: ' "$MIN_LOG"; then
                bad "ca âm: ảnh TỐI GIẢN lại có shell/uart ⇒ bộ kiểm không tách được artefact"
            else
                good "ca âm: ảnh tối giản không shell/uart (bộ kiểm tách đúng artefact)"
            fi
        else
            bad "ca âm: ảnh tối giản không boot sạch (log $MIN_LOG)"
        fi
    fi
fi

echo
echo "# product-x86_64: checks=$CHECKS fails=$FAILS logdir=$LOGDIR"
[ "$FAILS" -eq 0 ] || exit 1
exit 0
