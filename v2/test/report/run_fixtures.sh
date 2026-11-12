#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Fixture âm cho máy kiểm báo cáo (Khối E): mỗi ca PHẢI bị bắt.
set -u
cd "$(dirname "$0")/../../.." || exit 1
PY="${PYTHON:-python3}"
echo "1..5"
n=0; bad=0
run() {
    n=$((n + 1))
    if "$PY" v2/test/report/check_report.py --file "v2/test/report/fixtures/$1.json" --json-only >/tmp/repfx.out 2>&1; then
        echo "not ok $n - fixture $1 KHÔNG bị bắt"; bad=$((bad + 1))
    elif grep -q "$2" /tmp/repfx.out; then
        echo "ok $n - fixture $1 bị bắt"
    else
        echo "not ok $n - fixture $1 bị bắt nhưng sai lý do"; sed 's/^/    /' /tmp/repfx.out; bad=$((bad + 1))
    fi
}
run bad-head "head"
run bad-commit "commit"
run bad-status "≠ QUEUE"
run missing-task "thiếu task"
run bad-yc "YC-99"
if [ "$bad" -ne 0 ]; then echo "# fixtures: FAIL ($bad)"; exit 1; fi
echo "# fixtures: PASS"
