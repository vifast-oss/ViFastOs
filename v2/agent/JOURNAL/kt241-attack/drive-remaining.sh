#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-241 — chạy 8 ô còn lại (D5, E1..E7) TUẦN TỰ, lưu log từng ô + gộp bảng kết quả.
# run_matrix() của run.sh truncate results/current.tsv mỗi lần gọi ⇒ tự gộp ở đây.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." || exit 3
ROOT="$PWD"
AT="$ROOT/v2/agent/JOURNAL/kt241-attack"
export PATH="$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN=1.100.0

CELLS=(D5 E1 E2 E3 E4 E5 E6 E7)
ROWS="$AT/results/rows.tsv"
: > "$ROWS"

for c in "${CELLS[@]}"; do
    mkdir -p "$AT/logs/$c"
    echo "########## $c ##########"
    bash "$AT/run.sh" --cell "$c" --with-depinfo > "$AT/logs/$c/run.log" 2>&1
    rc=$?
    grep -E "^$c[[:space:]]" "$AT/logs/$c/run.log" >> "$ROWS"
    grep -E "^      ↳" "$AT/logs/$c/run.log" >> "$AT/logs/$c/detail.txt" 2>/dev/null || true
    echo "$c rc=$rc :: $(grep -E "^$c[[:space:]]" "$AT/logs/$c/run.log" | awk '{print $NF}')"
    # cây phải sạch sau mỗi ô (chỉ cho phép tệp log/kết quả chưa theo dõi)
    dirty="$(git status --porcelain -- ':!v2/agent/JOURNAL/kt241-attack' | grep -v '^??' || true)"
    if [[ -n "$dirty" ]]; then
        echo "!!! CÂY BẨN SAU $c:"; printf '%s\n' "$dirty"
    fi
done

{
    printf '%-3s %-2s %-9s %-6s %-14s %-6s %-14s %-6s %-14s %-6s %-14s %-6s %s\n' \
        ID GRP KIND CARGO CODES L1 L1CODES L1S L1SCODES L2 L2CODES POS VERDICT
    cat "$ROWS"
} > "$AT/results/current.tsv"

echo "== XONG =="
cat "$AT/results/current.tsv"
