#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
set -u
cd "$(dirname "$0")/../../.." || exit 1
if [ -z "${PYTHON:-}" ]; then
    for c in python3.12 python3.11 python3; do
        if command -v "$c" >/dev/null 2>&1; then PY="$c"; break; fi
    done
else
    PY="$PYTHON"
fi
echo "1..5"
n=0; bad=0
run() {
    n=$((n + 1))
    if "$PY" viscript/test/report/check_report.py --file "viscript/test/report/fixtures/$1.json" --json-only >/tmp/vrepfx.out 2>&1; then
        echo "not ok $n - fixture $1 KHÔNG bị bắt"; bad=$((bad + 1))
    elif grep -q "$2" /tmp/vrepfx.out; then
        echo "ok $n - fixture $1 bị bắt"
    else
        echo "not ok $n - fixture $1 bị bắt nhưng sai lý do"; sed 's/^/    /' /tmp/vrepfx.out; bad=$((bad + 1))
    fi
}
run bad-head "head"
run bad-commit "commit"
run bad-status "queue"
run missing-task "thiếu task"
run bad-project "project"
if [ "$bad" -ne 0 ]; then echo "# fixtures: FAIL ($bad)"; exit 1; fi
echo "# fixtures: PASS"
