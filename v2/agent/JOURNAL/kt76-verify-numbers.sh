#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-76 — kiểm **số đo** trong tài liệu của cổng K6 theo D2-109C/D2-110A/D2-112 (+D2-112E).
# Chi tiết luật: xem đầu `kt76_verify_numbers.py`.
#
# Dùng:
#   bash v2/agent/JOURNAL/kt76-verify-numbers.sh                 # kiểm cây thật
#   KT76_ROOT=<bản-copy>/v2 bash ...                             # kiểm bản copy (tự phá, D2-113F)
# Exit: 0 = mọi số có nguồn; 1 = có số không có nguồn (in `fail=N` + từng dòng).
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
ROOT="${KT76_ROOT:-$V2_ROOT}"

PY=""
for c in python3.12 python3.11 python3; do
    if command -v "$c" >/dev/null 2>&1; then PY="$c"; break; fi
done
[ -n "$PY" ] || { echo "kt76-verify-numbers: không có python3" >&2; exit 2; }

"$PY" "$HERE/kt76_verify_numbers.py" "$ROOT"
