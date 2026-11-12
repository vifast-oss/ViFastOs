#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Kiểm gói Agent 0: chạy từ gốc repo, không sửa gì.
set -uo pipefail

cd "$(dirname "$0")/../.." || exit 2

PY=""
for c in python3.12 python3.13 python3.11 python3; do
    if command -v "$c" >/dev/null 2>&1 && "$c" -c 'import tomllib' >/dev/null 2>&1; then
        PY="$c"
        break
    fi
done
if [ -z "$PY" ]; then
    echo "KIEM: không có Python ≥3.11 với tomllib" >&2
    exit 2
fi

fail=0
step() {
    echo
    echo "== $* =="
    "$@" || fail=$((fail + 1))
}

step "$PY" v2/test/scn/check_scn.py --dir v2/test/scn --spec spec/abi.toml
step bash v2/test/scn/run_fixtures.sh
step "$PY" v2/test/abi/check_abi.py --file spec/abi.toml
step "$PY" v2/test/abi/check_reserved.py --file spec/abi.toml
step ./v2/bin/v2 check
step ./v2/bin/v2 gate --fast

echo
if [ "$fail" -eq 0 ]; then
    echo "KIEM: PASS (toàn bộ)"
else
    echo "KIEM: $fail bước đỏ — đọc từng mục ở trên; đỏ trước khi Agent 2 áp D2-29/32/33 là bình thường"
fi
exit "$fail"
