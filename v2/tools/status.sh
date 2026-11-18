#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-102** — `v2 status`: một màn hình trạng thái dòng OS, để bàn giao và để Agent 0 đọc nhanh.
#
# Chỉ **đọc**; không build, không chạy QEMU, không sửa gì. Mục tiêu < 2 s.
#
# Dùng: bash v2/tools/status.sh [--json]
set -u
LC_ALL=C
export LC_ALL

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$V2_ROOT/.." && pwd)"
JSON=0
[ "${1:-}" = "--json" ] && JSON=1

cd "$ROOT" || exit 2

HEAD="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
HEAD12="$(printf '%s' "$HEAD" | cut -c1-12)"
BRANCH="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown)"
UNPUSHED="$(git rev-list --count origin/main..HEAD 2>/dev/null || echo '?')"

# shellcheck source=/dev/null
. "$V2_ROOT/test/boot/build-rev.sh"
FP="unknown"
COUNT=0
if vbrev_inputs_calc "$ROOT" >/dev/null 2>&1; then
    FP="$VBREV_CALC_SHORT"
    COUNT="$VBREV_CALC_COUNT"
fi
DIRTY="$(vbrev_dirty "$ROOT" 2>/dev/null || echo '?')"

# Ảnh mới nhất trong artifacts/ (theo mtime), kèm dòng danh tính.
ART=""
ART_SHA=""
ART_SIZE=""
ART_LINE=""
if [ -d "$V2_ROOT/artifacts" ]; then
    ART="$(ls -t "$V2_ROOT/artifacts"/*.bin 2>/dev/null | head -1)"
fi
if [ -n "$ART" ]; then
    ART_SHA="$(shasum -a 256 "$ART" | awk '{print $1}')"
    ART_SIZE="$(wc -c <"$ART" | tr -d ' ')"
    # KT-143: dạng mới có `algo=`; vẫn nhận dạng cũ để ảnh lịch sử vẫn hiện đúng dòng.
    ART_LINE="$(LC_ALL=C grep -a -o -m1 -e '# build: algo=[0-9A-Za-z._-]* inputs=[0-9a-f]* dirty=[01]' -e '# build: inputs=[0-9a-f]* dirty=[01]' -e '# build: rev=[0-9a-f]* dirty=[01]' "$ART" 2>/dev/null || true)"
fi

# Đếm task theo trạng thái trong QUEUE.
Q="$V2_ROOT/agent/QUEUE.md"
n_done=0
n_ready=0
n_blocked=0
n_review=0
if [ -f "$Q" ]; then
    n_done="$(grep -cE '^\| [A-Z]+-[0-9]+[a-z]? \|.*\| DONE \|' "$Q" || true)"
    n_ready="$(grep -cE '^\| [A-Z]+-[0-9]+[a-z]? \|.*\| READY \|' "$Q" || true)"
    n_blocked="$(grep -cE '^\| [A-Z]+-[0-9]+[a-z]? \|.*\| BLOCKED \|' "$Q" || true)"
    n_review="$(grep -cE '^\| [A-Z]+-[0-9]+[a-z]? \|.*\| REVIEW \|' "$Q" || true)"
fi

if [ "$JSON" -eq 1 ]; then
    printf '{"head":"%s","branch":"%s","unpushed":%s,"fingerprint":"%s","inputs":%s,"dirty":"%s","artifact":"%s","artifact_sha":"%s","artifact_size":"%s","artifact_line":"%s","tasks":{"done":%s,"ready":%s,"blocked":%s,"review":%s}}\n' \
        "$HEAD" "$BRANCH" "$UNPUSHED" "$FP" "$COUNT" "$DIRTY" "$ART" "$ART_SHA" "$ART_SIZE" "$ART_LINE" \
        "$n_done" "$n_ready" "$n_blocked" "$n_review"
    exit 0
fi

echo "== dòng OS ViFastOs =="
echo "HEAD        $HEAD12 ($BRANCH)  chưa push: $UNPUSHED"
echo "build-inputs $COUNT mục  dấu vân tay: $FP  dirty: $DIRTY"
if [ -n "$ART" ]; then
    echo "ảnh mới nhất $(basename "$ART")"
    echo "             sha256=$ART_SHA size=$ART_SIZE"
    echo "             $ART_LINE"
else
    echo "ảnh mới nhất (không có trong v2/artifacts/)"
fi
echo "task        DONE=$n_done READY=$n_ready BLOCKED=$n_blocked REVIEW=$n_review"
echo "cổng        ./v2/bin/v2 check | ./v2/bin/v2 check-rules | ./v2/bin/v2 gate --fast"
