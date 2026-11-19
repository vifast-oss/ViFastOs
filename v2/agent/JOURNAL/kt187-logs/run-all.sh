#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# KT-187 / L25 — harness độc lập của reviewer cho KT-184 (D2-160 + D2-161).
# Mọi đột biến nằm trong thư mục tạm; **không** sửa cây thật.
#
# Chạy: bash v2/agent/JOURNAL/kt187-logs/run-all.sh
# Ghi:  *.log cùng thư mục.
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../../.." && pwd)
BASE="${TMPDIR:-/tmp}/kt187-l25"
PY=python3
CUR="$REPO/v2/tools/check-wake-paths.py"
PRE="$BASE/check-pre184.py"      # 9921f606^ = 7723b46c (đã có D2-161, chưa có luật test-only)
OLD="$BASE/check-kt174.py"       # 7e7e1b53 (KT-174, TRƯỚC fixpoint KT-177)
B6="$BASE/check-b6efc403.py"     # b6efc403 (base worktree KT-184, TRƯỚC D2-161)

rm -rf "$BASE"
mkdir -p "$BASE"
git -C "$REPO" show 7e7e1b53:v2/tools/check-wake-paths.py > "$OLD"
git -C "$REPO" show 9921f606^:v2/tools/check-wake-paths.py > "$PRE"
git -C "$REPO" show b6efc403:v2/tools/check-wake-paths.py > "$B6"

build_tree() {
    rm -rf "$1"
    mkdir -p "$1/v2"
    cp -R "$REPO/v2/kernel" "$1/v2/kernel"
    cp -R "$REPO/v2/sim" "$1/v2/sim"
    cp -R "$REPO/v2/lib" "$1/v2/lib"
}

run() {
    label="$1"; checker="$2"; root="$3"; shift 3
    out=$("$PY" "$checker" --root "$root" "$@" 2>&1)
    rc=$?
    echo "### $label"
    echo "$out" | grep -E '^E |^# check-wake' || true
    echo "rc=$rc"
    echo
}

export REPO BASE PY CUR PRE OLD B6

bash "$HERE/t1-h4.sh"    > "$HERE/h4.log"    2>&1
bash "$HERE/t2-d2161.sh" > "$HERE/d2161.log" 2>&1
bash "$HERE/t3-bypass.sh" > "$HERE/bypass.log" 2>&1
"$PY" "$HERE/t4-teeth.py" > "$HERE/teeth.log" 2>&1
bash "$HERE/t5-h6h7.sh"  > "$HERE/h6h7.log"  2>&1
bash "$HERE/t6-fmt.sh"   > "$HERE/fmt.log"   2>&1
bash "$HERE/t7-compile.sh" > "$HERE/t7-compile.out" 2>&1
bash "$HERE/t8-rustc.sh"   > "$HERE/t8-rustc.out"   2>&1
echo "xong: $HERE/*.log"
