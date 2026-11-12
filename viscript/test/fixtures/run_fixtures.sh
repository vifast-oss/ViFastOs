#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Case âm bắt buộc của ST-01: mỗi luật của `vis-dev` phải CHỨNG MINH nó bắt được vi phạm.
set -u
VIS_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DEV="$VIS_ROOT/bin/vis-dev"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
echo "1..4"
n=0; bad=0
expect_fail() {
    n=$((n + 1))
    local name="$1" want="$2" dir="$3"
    if bash "$DEV" rules --root "$dir" >"$tmp/out" 2>&1; then
        echo "not ok $n - $name KHÔNG bị bắt"; bad=$((bad + 1))
    elif grep -q "$want" "$tmp/out"; then
        echo "ok $n - $name bị bắt ($want)"
    else
        echo "not ok $n - $name bị bắt nhưng sai lý do"; sed 's/^/    /' "$tmp/out"; bad=$((bad + 1))
    fi
}
# 1) thiếu SPDX
mkdir -p "$tmp/1"; printf 'fn x() {}\n' > "$tmp/1/bad.rs"
expect_fail "thiếu SPDX" "R3" "$tmp/1"
# 2) tệp > 1.500 dòng
mkdir -p "$tmp/2"; { echo '// SPDX-License-Identifier: MIT OR Apache-2.0'; for i in $(seq 1 1501); do echo "// dòng $i"; done; } > "$tmp/2/long.rs"
expect_fail "tệp 1.501 dòng" "R1" "$tmp/2"
# 3) dùng unsafe
mkdir -p "$tmp/3"; printf '// SPDX-License-Identifier: MIT OR Apache-2.0\nunsafe fn x() {}\n' > "$tmp/3/u.rs"
expect_fail "unsafe" "R2" "$tmp/3"
# 4) vis-host-vifast nằm trong viscript/
mkdir -p "$tmp/4/vis-host-vifast"; printf '# SPDX-License-Identifier: MIT OR Apache-2.0\n' > "$tmp/4/vis-host-vifast/Cargo.toml"
expect_fail "vis-host-vifast trong viscript/" "R4" "$tmp/4"
if [ "$bad" -ne 0 ]; then echo "# fixtures: FAIL ($bad)"; exit 1; fi
echo "# fixtures: PASS (cây sạch phải xanh)"
