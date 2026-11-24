#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-79 (D2-109C/D2-110A/D2-112) — mọi số trong tài liệu KT-79 phải xuất hiện **nguyên văn**
# trong một log của chính task đã nằm trong repo (`v2/agent/JOURNAL/kt79-*.log`).
#
# Phạm vi (ghi rõ, không allowlist): tài liệu `v2/docs/k6-repro.md` + khối `## 26.` trong
# `v2/docs/k3b-core.md`. Token bị kiểm = chuỗi hex >= 8 ký tự (sha/hash) + số thập phân >= 3 chữ
# số. Số 1–2 chữ số (số hiệu tiết, "2 máy", "8 B"...) không kiểm — chúng không phải số đo.
#
# Dùng: bash kt79-verify-numbers.sh [--docs-root DIR]
#   --docs-root DIR : kiểm trên **bản copy** (selfbreak không bao giờ sửa cây thật, D2-113F).
# Trả 0 nếu mọi token có trong log; 1 nếu thiếu (in danh sách thiếu).
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
JOURNAL="$HERE"
REPO_ROOT="$(cd "$HERE/../../.." && pwd)"
DOCS_ROOT="$REPO_ROOT"

while [ $# -gt 0 ]; do
    case "$1" in
        --docs-root) [ -n "${2:-}" ] || { echo "thiếu DIR" >&2; exit 2; }; DOCS_ROOT="$2"; shift 2 ;;
        *) echo "tham số không biết: $1" >&2; exit 2 ;;
    esac
done

DOCS=("$DOCS_ROOT/v2/docs/k6-repro.md")
[ -f "$DOCS_ROOT/v2/docs/k3b-core.md" ] && DOCS+=("$DOCS_ROOT/v2/docs/k3b-core.md")

# Haystack = hợp các log của task (KHÔNG gộp log mutation/selfbreak của task khác).
HAYSTACK="$(mktemp)"
LOGS=0
for f in "$JOURNAL"/kt79*.log; do
    [ -f "$f" ] || continue
    LOGS=$((LOGS + 1))
    cat "$f" >>"$HAYSTACK"
done
if [ "$LOGS" -eq 0 ]; then
    echo "kt79-verify-numbers: không thấy log kt79-*.log trong $JOURNAL" >&2
    exit 2
fi

missing=0
checked=0
for d in "${DOCS[@]}"; do
    [ -f "$d" ] || { echo "kt79-verify-numbers: thiếu $d" >&2; exit 2; }
    # Với k3b-core: chỉ lấy từ dòng '## 26.' tới hết (khối của KT-79).
    body="$d"
    if [ "${d##*/}" = "k3b-core.md" ]; then
        body="$(mktemp)"
        sed -n '/^## 26\./,$p' "$d" >"$body"
    fi
    # token: hex >= 8 hoặc số >= 3 chữ số
    for tok in $(grep -oE '[0-9a-f]{8,}|[0-9]{3,}' "$body" | sort -u); do
        # Ngoại lệ ĐỊNH DANH (không phải số đo), ghi lý do từng mục — D2-112(b):
        #   108/118/119/120/175 = số hiệu quyết định/task (D2-108A, D2-118, D2-119, D2-120, A1-175).
        case "$tok" in 108 | 118 | 119 | 120 | 175) continue ;; esac
        checked=$((checked + 1))
        grep -qF -- "$tok" "$HAYSTACK" || {
            echo "MISSING $tok  (tài liệu ${d#$DOCS_ROOT/})"
            missing=$((missing + 1))
        }
    done
    [ "$body" = "$d" ] || rm -f "$body"
done
rm -f "$HAYSTACK"

echo "kt79-verify-numbers: docs=${#DOCS[@]} logs=$LOGS tokens=$checked missing=$missing"
[ "$missing" -eq 0 ]
