#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-241 — chạy 4 ô sau khi sửa kỳ vọng manifest + thêm D6 (C3, D5, E7, D6). TUẦN TỰ.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." || exit 3
ROOT="$PWD"; AT="$ROOT/v2/agent/JOURNAL/kt241-attack"
export PATH="$HOME/.cargo/bin:$PATH"; export RUSTUP_TOOLCHAIN=1.100.0
CELLS=(C3 D5 E7 D6)
ROWS="$AT/results/rows-fix.tsv"; : > "$ROWS"
for c in "${CELLS[@]}"; do
    mkdir -p "$AT/logs/$c"
    echo "########## $c ##########"
    bash "$AT/run.sh" --cell "$c" --with-depinfo > "$AT/logs/$c/run.log" 2>&1
    rc=$?
    grep -E "^$c[[:space:]]" "$AT/logs/$c/run.log" >> "$ROWS"
    grep -E "^      ↳" "$AT/logs/$c/run.log" > "$AT/logs/$c/detail.txt" 2>/dev/null || true
    echo "$c rc=$rc :: $(grep -E "^$c[[:space:]]" "$AT/logs/$c/run.log" | awk '{print $NF}')"
    dirty="$(git status --porcelain -- ':!v2/agent/JOURNAL/kt241-attack' | grep -v '^??' || true)"
    [[ -n "$dirty" ]] && { echo "!!! CÂY BẨN SAU $c:"; printf '%s\n' "$dirty"; }
done
echo "== XONG =="; cat "$ROWS"
