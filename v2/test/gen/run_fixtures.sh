#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Case âm của gen-check: sửa tay code/docs, hoặc spec thiếu chữ ký, đều PHẢI bị bắt.
set -u
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
PY=""
for c in python3.12 python3.11 python3; do command -v "$c" >/dev/null 2>&1 && { PY="$c"; break; }; done
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
gen() { "$PY" "$ROOT/v2/tools/vifast-gen/gen.py" "$@"; }
mk_tree() {
    mkdir -p "$tmp/$1/spec" "$tmp/$1/v2/lib/vifast-abi/src" "$tmp/$1/docs"
    cp "$ROOT/spec/abi.toml" "$tmp/$1/spec/"
    cp "$ROOT/v2/lib/vifast-abi/src/generated.rs" "$tmp/$1/v2/lib/vifast-abi/src/"
    cp "$ROOT/docs/abi.md" "$tmp/$1/docs/"
}
echo "1..4"
n=0; bad=0
expect_fail() {
    n=$((n + 1)); local name="$1" want="$2" dir="$3"
    if gen --check --root "$dir" --spec "$dir/spec/abi.toml" >"$tmp/out" 2>&1; then
        echo "not ok $n - $name KHÔNG bị bắt"; bad=$((bad + 1))
    elif grep -q "$want" "$tmp/out"; then
        echo "ok $n - $name bị bắt"
    else
        echo "not ok $n - $name bị bắt nhưng sai lý do"; sed 's/^/    /' "$tmp/out"; bad=$((bad + 1))
    fi
}
# 1) sửa tay code sinh
mk_tree a; echo "// sửa tay" >> "$tmp/a/v2/lib/vifast-abi/src/generated.rs"
expect_fail "sửa tay generated.rs" "LỆCH" "$tmp/a"
# 2) sửa tay docs/abi.md
mk_tree b; echo "sửa tay" >> "$tmp/b/docs/abi.md"
expect_fail "sửa tay docs/abi.md" "LỆCH" "$tmp/b"
# 3) spec thiếu [method_signature]
mk_tree c
"$PY" - "$tmp/c/spec/abi.toml" <<'PY'
import re, sys
src = open(sys.argv[1]).read()
out = re.sub(r"\[method_signature\].*", "", src, flags=re.S)
open(sys.argv[1], "w").write(out)
PY
expect_fail "spec thiếu method_signature" "stable-draft" "$tmp/c"
# 4) đầu ra phải ĐỔI khi spec đổi abi_version (chống gen tự chứng)
n=$((n + 1))
mk_tree d
"$PY" - "$tmp/d/spec/abi.toml" <<'PY2'
import sys
src = open(sys.argv[1]).read()
out = src.replace("abi_version = 1", "abi_version = 2", 1) if "abi_version = 1" in src else src
open(sys.argv[1], "w").write(out)
PY2
if gen --root "$tmp/d" --spec "$tmp/d/spec/abi.toml" >"$tmp/out4" 2>&1 && grep -q "ABI_VERSION: u32 = 2" "$tmp/d/v2/lib/vifast-abi/src/generated.rs"; then
    echo "ok $n - abi_version đổi thì đầu ra đổi"
else
    echo "not ok $n - abi_version đổi mà đầu ra KHÔNG đổi"; sed 's/^/    /' "$tmp/out4"; bad=$((bad + 1))
fi
if [ "$bad" -ne 0 ]; then echo "# gen fixtures: FAIL ($bad)"; exit 1; fi
echo "# gen fixtures: PASS"
