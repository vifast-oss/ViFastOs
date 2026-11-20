#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-55 — chạy conformance 5 `.scn` × sim/QEMU + so trace.
#
# Hai backend:
#   1. **sim (host)**: `cargo test -p vifast-sim --test kt42_scn -- --nocapture --test-threads=1`
#      — runner KT-42 chạy **mọi** bước của 5 tệp trên mô hình K2 (0 lệch), chỉ in `# trace:` cho
#      tập con conformance do `vifast-scenario` chiếu (cùng logic với kernel).
#   2. **QEMU (ảnh ktest)**: `bash v2/test/boot/ktest.sh 1 matrix` — pha conformance in cùng
#      định dạng; so log **smp≥2** (ca 24 cần 2 hart thật).
#
# Dùng:
#   bash v2/test/conformance/run.sh --sim-only          # chỉ lấy trace sim
#   bash v2/test/conformance/run.sh --compare [--smp 2] # sim + log QEMU có sẵn
#   bash v2/test/conformance/run.sh --boot [--smp 2]    # sim + boot QEMU + so
#   KT55_LOGDIR=<dir> đổi nơi ghi log (mặc định /tmp/k3-kt55)
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
LOGDIR="${KT55_LOGDIR:-${V2_RUN_TMP:-${TMPDIR:-/tmp}}/k3-kt55}"
SMP=2
MODE=compare
BOOT=0

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

die() {
    echo "run.sh: $*" >&2
    exit 2
}

while [ $# -gt 0 ]; do
    case "$1" in
        --sim-only) MODE=sim ;;
        --compare) MODE=compare ;;
        --boot) BOOT=1 ;;
        --smp)
            [ $# -ge 2 ] || die "--smp cần giá trị"
            SMP="$2"
            shift
            ;;
        --help)
            sed -n '3,20p' "${BASH_SOURCE[0]}" | sed -e 's/^# \{0,1\}//'
            exit 0
            ;;
        *) die "tham số lạ: $1" ;;
    esac
    shift
done

mkdir -p "$LOGDIR"
SIM_RAW="$LOGDIR/sim-kt42.log"
SIM_TRACE="$LOGDIR/sim-trace.log"
QEMU_LOG="$LOGDIR/ktest-smp${SMP}-1.log"
CMP_LOG="$LOGDIR/trace-compare.log"

# 1) sim — một luồng để các dòng trace **không xen** giữa các ca.
echo "# sim: cargo test -p vifast-sim --test kt42_scn"
if ! (cd "$V2_ROOT" && cargo test -p vifast-sim --test kt42_scn -- --nocapture --test-threads=1) \
    >"$SIM_RAW" 2>&1; then
    echo "not ok - sim: kt42_scn FAIL (xem $SIM_RAW)"
    grep -E '^not ok|panicked|FAILED|error' "$SIM_RAW" | head -10
    exit 1
fi
grep -oE '# (trace|conf): .*' "$SIM_RAW" >"$SIM_TRACE" || true
n_sim="$(grep -c '^# trace: ' "$SIM_TRACE" || true)"
echo "# sim trace: ${n_sim} dòng → $SIM_TRACE"
[ "${n_sim:-0}" -gt 0 ] || die "sim không in dòng trace nào"

# 1b) kiểm **chiều sim** bằng máy kiểm `.scn` (định dạng + tập bước chiếu + giá trị quan sát).
PY_BIN="${PYTHON:-python3.12}"
if ! (cd "$V2_ROOT/.." && "$PY_BIN" v2/test/scn/check_scn.py --trace "$SIM_TRACE"); then
    echo "not ok - check_scn --trace FAIL (chiều sim)"
    exit 1
fi

[ "$MODE" = sim ] && {
    echo "ok - sim trace sẵn sàng"
    exit 0
}

# 2) QEMU — log của runner KT-54 (không tự build lại nếu không xin `--boot`).
if [ "$BOOT" = 1 ]; then
    echo "# qemu: ktest.sh 1 matrix (4 ảnh × smp 1/2/4)"
    if ! (cd "$V2_ROOT" && bash test/boot/ktest.sh 1 matrix) >"$LOGDIR/ktest-matrix.log" 2>&1; then
        echo "not ok - ktest.sh matrix FAIL (xem $LOGDIR/ktest-matrix.log)"
        tail -20 "$LOGDIR/ktest-matrix.log"
        exit 1
    fi
fi
KT_LOGDIR="${KT54_LOGDIR:-${V2_RUN_TMP:-${TMPDIR:-/tmp}}/k3-ktest}"
[ -f "$QEMU_LOG" ] || QEMU_LOG="$KT_LOGDIR/ktest-smp${SMP}-1.log"
[ -f "$QEMU_LOG" ] || die "không thấy log QEMU cho smp=$SMP (chạy --boot hoặc KT54_LOGDIR)"

# 3) so — chỉ các trường được phép (`trace_compare.py`).
KT_IMG="$V2_ROOT/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest"
[ -f "$KT_IMG" ] || die "không thấy ảnh ktest ($KT_IMG) — chạy --boot trước"
if python3 "$HERE/trace_compare.py" --sim "$SIM_TRACE" --qemu "$QEMU_LOG" --image "$KT_IMG" \
    >"$CMP_LOG" 2>&1; then
    cat "$CMP_LOG"
    echo "# trace-compare PASS ($SIM_TRACE ↔ $QEMU_LOG)"
    exit 0
fi
cat "$CMP_LOG"
echo "# trace-compare FAIL — log: $CMP_LOG"
exit 1
