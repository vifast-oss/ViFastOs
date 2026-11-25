#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# `bench-bkl` — **đo tranh khoá Big Kernel Lock trên N hart** (KT-415, D2-259 mục 6).
#
# Vì sao tệp này ở `v2/tools/` chứ không phải `v2/bin/vifast-bench`: `D2-253` chốt **chỉ lead**
# được sửa `v2/bin/**`; sub-agent phải báo lead. Tệp này là phần **đo** tách rời, dùng đúng bộ
# phân tích telemetry chuẩn (`v2/tools/telemetry.py`) và đúng khung `-icount` của
# `v2/bin/vifast-bench`; lead chỉ cần thêm một nhánh `--bkl` gọi tệp này là nối xong.
#
# # Đo cái gì
#
# Ảnh sản phẩm riscv64 build với feature `bench-bkl`: sau khi bring-up SMP thật (`smp::init`),
# **mỗi hart online** chạy `OPS` vòng vào vùng găng của Big Kernel Lock (`IrqMcsLock<Machine>`,
# `core_bridge::with_machine`). Bộ đếm `acquired`/`contended` của chính khoá được in thành
# `# bench: kind=bkl …` — **số đo**, không phải hằng viết tay (QD §9 Q12).
#
# Tỷ lệ chờ = `contended / acquired`, in dạng **phần triệu** (`ratio_ppm`). Hart đơn lẻ ⇒ 0;
# càng nhiều hart cùng vào vùng găng ⇒ càng cao. Đây là **đại lượng** làm tiêu chuẩn kích hoạt
# khoá Per-Hart / đa cụm `S3`.
#
# Dùng:
#   bash v2/tools/bench-bkl.sh [--smp N] [--csv <tệp>] [--no-build] [--icount] [--dry-run]
#
# # Vì sao mặc định **KHÔNG** `-icount` (khác `vifast-bench --smoke`)
#
# `-icount` làm QEMU chạy các vCPU **luân phiên** (một vCPU một lúc) và đồng hồ ảo tiến theo **số
# lệnh**. Đo được (KT-415): với `-icount`, 4 hart cho `contended=0` — mỗi hart chạy trọn lát của nó
# nên gần như **không bao giờ** chạm khoá đang bị giữ; tỷ lệ chờ **biến mất** (số đo SAI bản chất,
# không phải "ít tranh chấp"). Không `-icount` (TCG chạy song song thật) cho `contended/acquired`
# ≈ 97 %. Tỷ lệ chờ là **đại lượng không thứ nguyên** nên không cần đồng hồ ảo; `-icount` chỉ cần
# cho phép đo **thời gian**. Vì vậy mặc định tắt, `--icount` để tái lập phép đo đối chứng.
#
# Exit: 0 = đo xong (mọi hart báo DONE, evidence hợp quy chuẩn); 1 = boot/evidence đỏ;
#       2 = môi trường (cargo/QEMU/OpenSBI/python) thiếu; 3 = TỪ CHỐI ĐO (tham số sai).
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/.." && pwd)"
BOOT_LIB="$V2_ROOT/test/boot/lib/env.sh"
CSV="${V2_BKL_CSV:-$V2_ROOT/test/perf/bkl.csv}"
CSV_HEADER="measure,build_id,arch,mode,icount,harts,ops,acquired,contended,ratio_ppm,wall_ms,qemu_rc,ok"

PY=""
for c in python3.12 python3.11 python3; do
    if command -v "$c" >/dev/null 2>&1 && "$c" -c 'import tomllib' >/dev/null 2>&1; then
        PY="$(command -v "$c")"
        break
    fi
done

ICount="shift=auto,align=off,sleep=off"
ICountCSV="$(printf '%s' "$ICount" | tr ',' '+')"

die() {
    echo "bench-bkl: $*" >&2
    exit 2
}

usage() {
    sed -n '2,26p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

SMP=4
DO_BUILD=1
DRY=0
USE_ICOUNT=0
while [ $# -gt 0 ]; do
    case "$1" in
        --smp) shift; SMP="${1:?--smp cần giá trị}" ;;
        --csv) shift; CSV="${1:?--csv cần giá trị}" ;;
        --no-build) DO_BUILD=0 ;;
        --icount) USE_ICOUNT=1 ;;
        --dry-run) DRY=1 ;;
        --help | -h) usage; exit 0 ;;
        *) die "tham số lạ: $1 (--smp|--csv|--no-build|--icount|--dry-run)" ;;
    esac
    shift
done

case "$SMP" in
    '' | *[!0-9]*) die "--smp phải là số nguyên dương (nhận '$SMP')" ;;
esac
[ "$SMP" -ge 1 ] && [ "$SMP" -le 8 ] || die "--smp ngoài 1..8 (MAX_HARTS=8)"
[ -n "$PY" ] || die "cần python ≥3.11 có tomllib"

# ---------------------------------------------------------------- dry-run (không QEMU)
if [ "$DRY" -eq 1 ]; then
    fail=0
    n=0
    say() {
        n=$((n + 1))
        if [ "$1" = ok ]; then echo "ok $n - $2"; else echo "not ok $n - $2"; fail=$((fail + 1)); fi
    }
    echo "TAP version 13"
    echo "1..4"
    case "$ICount" in
        *shift=*align=*sleep=*) say ok "-icount có shift/align/sleep" ;;
        *) say fail "-icount thiếu thành phần" ;;
    esac
    [ "$CSV_HEADER" = "measure,build_id,arch,mode,icount,harts,ops,acquired,contended,ratio_ppm,wall_ms,qemu_rc,ok" ] &&
        say ok "schema bkl.csv khớp" || say fail "schema bkl.csv lệch"
    tmp="$(mktemp "${TMPDIR:-/tmp}/kt415-bkl.XXXXXX")" || die "mktemp lỗi"
    printf '%s\nbkl,abc123,riscv64,bkl,%s,4,80000,80010,1234,15423,0,0,1\n' "$CSV_HEADER" "$ICountCSV" >"$tmp"
    if "$PY" - "$tmp" <<'PYEOF'
import csv, sys
rows = list(csv.DictReader(open(sys.argv[1])))
assert rows and rows[0]["measure"] == "bkl" and rows[0]["icount"].count("=") == 3, rows
assert int(rows[0]["ratio_ppm"]) == 15423 and int(rows[0]["harts"]) == 4, rows
PYEOF
    then
        say ok "round-trip bkl.csv (nhãn bkl + ratio_ppm)"
    else
        say fail "round-trip bkl.csv"
    fi
    rm -f "$tmp"
    [ -e "$tmp" ] && say fail "tệp tạm còn sót" || say ok "không rò rỉ tệp tạm"
    if [ "$fail" -ne 0 ]; then
        echo "# bench-bkl dry-run: $fail/$n lỗi"
        exit 1
    fi
    echo "# bench-bkl dry-run: $n/$n PASS (không QEMU)"
    exit 0
fi

TARGET="riscv64gc-unknown-none-elf"
# **Bắt buộc** dùng `CARGO_TARGET_DIR` khi có (worktree không được đẻ `target/` — ngân sách đĩa).
TARGET_DIR="${CARGO_TARGET_DIR:-$V2_ROOT/kernel/target}"
IMAGE="$TARGET_DIR/$TARGET/release/vifast-kbin-riscv64"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
LOGDIR="${TMPDIR:-/tmp}/vifast-bench-bkl-$$"
mkdir -p "$LOGDIR" || die "không tạo được $LOGDIR"

command -v cargo >/dev/null 2>&1 || die "không có cargo"
QEMU="$(command -v qemu-system-riscv64 || true)"
[ -n "$QEMU" ] || die "không có qemu-system-riscv64"
SBI="${OPENSBI:-}"
if [ -z "$SBI" ]; then
    for c in /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
        /usr/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
        /usr/local/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin; do
        [ -f "$c" ] && { SBI="$c"; break; }
    done
fi
[ -f "${SBI:-/nonexistent}" ] || die "không tìm thấy OpenSBI fw_dynamic (đặt OPENSBI=...)"

# shellcheck source=/dev/null
[ -f "$BOOT_LIB" ] && . "$BOOT_LIB"
# shellcheck source=/dev/null
. "$V2_ROOT/test/boot/qemu_guard.sh"
QG_LOG="$LOGDIR/qemu-guard.log"
qg_init || die "qemu_guard không khởi tạo được (thiếu perl?)"
trap 'qg_cleanup 2>/dev/null || true; rm -rf "$LOGDIR"' EXIT INT TERM HUP

if [ "$DO_BUILD" -eq 1 ]; then
    echo "# build ảnh bench ($TARGET, release, features=bench-bkl)"
    if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-riscv64 \
        --bin vifast-kbin-riscv64 --target "$TARGET" --release --features bench-bkl \
        >"$LOGDIR/build.log" 2>&1; then
        tail -15 "$LOGDIR/build.log" >&2
        die "build ảnh FAIL"
    fi
fi
[ -f "$IMAGE" ] || die "không thấy ảnh $IMAGE (chạy không --no-build)"

if [ "$USE_ICOUNT" -eq 1 ]; then
    MODE="bkl-icount"
    ICOUNT_COL="$ICountCSV"
    ICOUNT_ARGS=(-icount "$ICount")
else
    MODE="bkl"
    ICOUNT_COL="none"
    ICOUNT_ARGS=()
fi
echo "# bench-bkl: arch=riscv64 smp=$SMP mode=$MODE icount=$ICOUNT_COL (số ĐO, không phải smoke)"
LOG="$LOGDIR/boot.log"
T0_MS="$("$PY" -c 'import time;print(int(time.time()*1000))')"
if ! qg_boot "$LOG" 300 "$QEMU" -machine virt,aia=none,aclint=on -bios "$SBI" \
    -kernel "$IMAGE" -nographic -cpu rv64 -m 128M -smp "$SMP" -no-reboot \
    ${ICOUNT_ARGS[@]+"${ICOUNT_ARGS[@]}"}; then
    QRC="$QG_LAST_RC"
else
    QRC=0
fi
T1_MS="$("$PY" -c 'import time;print(int(time.time()*1000))')"
WALL_MS=$((T1_MS - T0_MS))

ok=1
grep -q '^ok 1 - k3 boot$' "$LOG" || { echo "not ok - thiếu 'ok 1 - k3 boot'" >&2; ok=0; }
grep -q '^# k3 halt$' "$LOG" || { echo "not ok - thiếu '# k3 halt'" >&2; ok=0; }
grep -q '^# build: ' "$LOG" || { echo "not ok - thiếu '# build:'" >&2; ok=0; }
[ "$QRC" -eq 0 ] || { echo "not ok - QEMU rc=$QRC (log $LOG)" >&2; ok=0; }

BUILD_ID="$(sed -n 's/^# build: .*inputs=\([0-9a-f]\{8,\}\).*/\1/p' "$LOG" | head -1)"
[ -n "$BUILD_ID" ] || BUILD_ID="unknown"

# **Bộ phân tích chuẩn** (KT-403): đọc evidence `key=value` theo **tên trường**, không theo thứ tự.
if ! "$PY" "$V2_ROOT/tools/telemetry.py" --file "$LOG" --prefix bench \
    --require kind --require harts --require ops --require acquired --require contended \
    --require ratio_ppm --require done >"$LOGDIR/telemetry.out" 2>&1; then
    echo "not ok - telemetry 'bench' sai quy chuẩn" >&2
    cat "$LOGDIR/telemetry.out" >&2
    ok=0
fi
FIELDS="$("$PY" - "$LOGDIR/telemetry.out" <<'PYEOF'
import json, sys
line = ""
for raw in open(sys.argv[1]):
    raw = raw.strip()
    if raw.startswith("{"):
        line = raw
if not line:
    print("")
    raise SystemExit(0)
d = json.loads(line)["fields"]
print(" ".join(f"{k}={d.get(k, '')}" for k in
      ("kind", "harts", "ops", "acquired", "contended", "in_cs", "overlap", "ratio_ppm", "done", "mask")))
PYEOF
)"
echo "# bench-bkl telemetry: $FIELDS"

get() { printf '%s\n' "$FIELDS" | tr ' ' '\n' | sed -n "s/^$1=//p" | head -1; }
KIND="$(get kind)"
HARTS="$(get harts)"
OPS="$(get ops)"
ACQ="$(get acquired)"
CON="$(get contended)"
RATIO="$(get ratio_ppm)"
DONE="$(get done)"
MASK="$(get mask)"

[ "$KIND" = "bkl" ] || { echo "not ok - kind='$KIND' (cần 'bkl')" >&2; ok=0; }
[ -n "$HARTS" ] && [ "$HARTS" -ge 1 ] 2>/dev/null || { echo "not ok - harts='$HARTS'" >&2; ok=0; }
[ -n "$OPS" ] && [ "$OPS" -gt 0 ] 2>/dev/null || { echo "not ok - ops='$OPS'" >&2; ok=0; }
[ -n "$ACQ" ] && [ "$ACQ" -gt 0 ] 2>/dev/null || { echo "not ok - acquired='$ACQ'" >&2; ok=0; }
[ -n "$RATIO" ] || { echo "not ok - thiếu ratio_ppm" >&2; ok=0; }
# Mọi hart phải báo DONE: `done` == `mask` (cùng bitmask) — nếu không, số đo chỉ của một phần hart.
if [ -n "$DONE" ] && [ -n "$MASK" ] && [ "$DONE" != "$MASK" ]; then
    echo "not ok - không phải mọi hart báo DONE (done=$DONE mask=$MASK)" >&2
    ok=0
fi

mkdir -p "$(dirname "$CSV")" || die "không tạo được $(dirname "$CSV")"
if [ ! -s "$CSV" ]; then
    printf '%s\n' "$CSV_HEADER" >"$CSV"
fi
printf 'bkl,%s,riscv64,%s,%s,%s,%s,%s,%s,%s,%d,%d,%d\n' \
    "$BUILD_ID" "$MODE" "$ICOUNT_COL" "${HARTS:-0}" "${OPS:-0}" "${ACQ:-0}" "${CON:-0}" "${RATIO:-0}" \
    "$WALL_MS" "$QRC" "$ok" >>"$CSV"
echo "# bench-bkl: build_id=$BUILD_ID wall_ms=$WALL_MS qemu_rc=$QRC ok=$ok"
echo "# bench-bkl: history=$CSV (nhãn measure=bkl)"

[ "$ok" -eq 1 ] || exit 1
echo "# bench-bkl: PASS — harts=$HARTS ops=$OPS acquired=$ACQ contended=$CON ratio_ppm=$RATIO"
exit 0
