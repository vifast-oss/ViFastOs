#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Case âm cho máy kiểm ABI (review #13: abi/reserved chưa có TAP âm).
set -u
cd "$(dirname "$0")/../../.." || exit 1
if [ -z "${PYTHON:-}" ]; then
    for c in python3.12 python3.11 python3; do
        if command -v "$c" >/dev/null 2>&1; then PY="$c"; break; fi
    done
else
    PY="$PYTHON"
fi
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
echo "1..3"
n=0; bad=0
g() { "$PY" "$@"; }
expect_fail() {
    n=$((n + 1))
    name="$1"; want="$2"; shift 2
    if g "$@" >"$tmp/out" 2>&1; then
        echo "not ok $n - $name KHÔNG bị bắt"; bad=$((bad + 1))
    elif grep -q "$want" "$tmp/out"; then
        echo "ok $n - $name bị bắt"
    else
        echo "not ok $n - $name bị bắt nhưng sai lý do"; sed 's/^/    /' "$tmp/out"; bad=$((bad + 1))
    fi
}
# 1) thiếu một syscall (0..15 không đủ)
g - "$tmp/1.toml" <<'PY'
import sys
lines = open("spec/abi.toml").read().splitlines(keepends=True)
out = "".join(l for l in lines if not l.startswith('13 = { name = "DebugHalt"'))
open(sys.argv[1], "w").write(out)
PY
expect_fail "thiếu syscall 13" "syscall phải là 0..15" v2/test/abi/check_abi.py --file "$tmp/1.toml"
# 2) dải reserved syscall sai (15..31)
g - "$tmp/2.toml" <<'PY'
import sys
src = open("spec/abi.toml").read()
out = src.replace("[syscall_reserved]\nfrom = 16", "[syscall_reserved]\nfrom = 15", 1)
open(sys.argv[1], "w").write(out)
PY
expect_fail "reserved syscall 15..31" "dải reserved phải 16..31" v2/test/abi/check_abi.py --file "$tmp/2.toml"
# 3) object reserved ra ngoài 9..16
g - "$tmp/3.toml" <<'PY'
import sys
src = open("spec/abi.toml").read()
out = src.replace("VSpace = { id = 9,", "VSpace = { id = 18,", 1)
open(sys.argv[1], "w").write(out)
PY
expect_fail "object reserved id 18" "ngoài dải reserved" v2/test/abi/check_reserved.py --file "$tmp/3.toml"
if [ "$bad" -ne 0 ]; then echo "# abi fixtures: FAIL ($bad)"; exit 1; fi
echo "# abi fixtures: PASS"
