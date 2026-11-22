#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Case âm của IDL gen-check (KT-69): sửa tay stub, hoặc sửa IDL mà không sinh lại, đều PHẢI bị bắt.
set -u
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
PY=""
for c in python3.12 python3.11 python3; do command -v "$c" >/dev/null 2>&1 && { PY="$c"; break; }; done
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
gen() { local dir="$1"; shift; "$PY" "$ROOT/v2/tools/vifast-gen/gen_idl.py" --root "$dir" "$@"; }
mk_tree() {
    mkdir -p "$tmp/$1/v2/lib/vifast-idl/idl" "$tmp/$1/v2/lib/vifast-idl/src" \
        "$tmp/$1/spec/protocols"
    cp "$ROOT"/v2/lib/vifast-idl/idl/*.vidl "$tmp/$1/v2/lib/vifast-idl/idl/"
    # **D2-242 nhóm 6 (6A)**: generator quét thêm `spec/protocols/` ⇒ cây giả phải có đủ ba IDL,
    # nếu không mọi ca sẽ đỏ vì "không có tệp" thay vì đúng lý do đang kiểm (LỆCH).
    cp "$ROOT"/spec/protocols/*.vidl "$tmp/$1/spec/protocols/"
    cp "$ROOT"/v2/lib/vifast-idl/src/generated*.rs "$tmp/$1/v2/lib/vifast-idl/src/"
}
echo "1..2"
bad=0
expect_fail() {
    local name="$1" dir="$2"
    if gen "$dir" --check >"$tmp/out" 2>&1; then
        echo "not ok $name KHÔNG bị bắt"; bad=$((bad + 1))
    elif grep -q "LỆCH" "$tmp/out"; then
        echo "ok $name bị bắt"
    else
        echo "not ok $name bị bắt nhưng sai lý do"; sed 's/^/    /' "$tmp/out"; bad=$((bad + 1))
    fi
}
# 1) sửa tay stub sinh
mk_tree a; echo "// sửa tay" >> "$tmp/a/v2/lib/vifast-idl/src/generated.rs"
expect_fail "sửa tay generated.rs" "$tmp/a"
# 2) sửa IDL (thêm method) mà không sinh lại
mk_tree b
"$PY" - "$tmp/b/v2/lib/vifast-idl/idl/ring.vidl" <<'PY'
import sys
path = sys.argv[1]
src = open(path).read()
extra = "\nmethod ping\n  param value u64\n  returns u32\n"
open(path, "w").write(src + extra)
PY
expect_fail "IDL đổi mà stub không đổi" "$tmp/b"
if [ "$bad" -ne 0 ]; then echo "# idl fixtures: FAIL ($bad)"; exit 1; fi
echo "# idl fixtures: PASS"
