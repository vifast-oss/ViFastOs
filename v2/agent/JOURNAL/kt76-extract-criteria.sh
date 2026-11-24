#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-76 — sinh/kiểm `kt76-criteria.txt` (tiêu chí DOD **nguyên văn** từ nguồn đọc-only).
# Chi tiết vì sao: xem đầu `kt76_extract_criteria.py`.
#
# Dùng: bash kt76-extract-criteria.sh           # in ra stdout
#       bash kt76-extract-criteria.sh --check   # so tệp đã commit với bản sinh lại (từng byte)
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../../.." && pwd)"

PY=""
for c in python3.12 python3.11 python3; do
    if command -v "$c" >/dev/null 2>&1; then PY="$c"; break; fi
done
[ -n "$PY" ] || { echo "kt76-extract-criteria: không có python3" >&2; exit 2; }

if [ "${1:-}" = "--check" ]; then
    if "$PY" "$HERE/kt76_extract_criteria.py" "$REPO" | diff -u "$HERE/kt76-criteria.txt" -; then
        echo "kt76-extract-criteria: OK (kt76-criteria.txt = bản sinh lại từ nguồn đọc-only)"
        exit 0
    fi
    echo "kt76-extract-criteria: FAIL — tệp đã commit KHÁC bản sinh lại từ nguồn đọc-only" >&2
    exit 1
fi

"$PY" "$HERE/kt76_extract_criteria.py" "$REPO"
