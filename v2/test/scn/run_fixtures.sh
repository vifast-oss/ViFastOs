#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-04c: 3 fixture âm — máy kiểm phải BẮT được, không chỉ "chạy không lỗi".
set -u
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
PY="${PYTHON:-python3.12}"
CHECK="$ROOT/v2/test/scn/check_scn.py"
fails=0
n=0
run() {
    n=$((n + 1))
    if "$PY" "$CHECK" --dir "$ROOT/v2/test/scn/fixtures/$1" >/tmp/scn-fix-$1.log 2>&1; then
        echo "not ok $n - fixture $1 phải bị bắt"
        fails=$((fails + 1))
    else
        echo "ok $n - fixture $1 bị bắt"
    fi
}
echo "TAP version 13"
echo "1..4"
run bad-method
run bad-error
run dup-name
run bad-arity
echo "1..4" >/dev/null
if [ "$fails" -eq 0 ]; then echo "# scn fixtures: PASS"; exit 0; fi
echo "# scn fixtures: FAIL ($fails)"; exit 1
