#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Case âm của IDL gen-check (KT-69): sửa tay stub, hoặc sửa IDL mà không sinh lại, đều PHẢI bị bắt.
#
# **KT-420** (D2-262) — mở thêm 4 lớp ca:
#   * bộ **6 `.vidl` chuẩn** (`ns`, `proc`, `fs`, `blk`, `socket`, `term`) phải có đủ + khai đủ
#     method hợp đồng + stub sinh tương ứng (`generated_<tên>.rs`) + `lib.rs` khai module;
#   * `--check` trên **cây thật** vẫn khớp (gen-check của cổng);
#   * **ca âm của cú pháp mở rộng**: `bytesN` sai cỡ, hai `reply cap`, cap sai loại, và quá 4 MR
#     đều phải bị generator **từ chối** (không im lặng cắt).
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
PY=""
for c in python3.12 python3.11 python3; do command -v "$c" >/dev/null 2>&1 && { PY="$c"; break; }; done
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
gen() { local dir="$1"; shift; "$PY" "$ROOT/v2/tools/vifast-gen/gen_idl.py" --root "$dir" "$@"; }
mk_tree() {
    mkdir -p "$tmp/$1/v2/lib/vifast-idl/idl" "$tmp/$1/v2/lib/vifast-idl/src" \
        "$tmp/$1/spec/protocols"
    cp "$ROOT"/v2/lib/vifast-idl/idl/*.vidl "$tmp/$1/v2/lib/vifast-idl/idl/"
    # **D2-242 nhóm 6 (6A)**: generator quét thêm `spec/protocols/` ⇒ cây giả phải có đủ các IDL,
    # nếu không mọi ca sẽ đỏ vì "không có tệp" thay vì đúng lý do đang kiểm (LỆCH).
    cp "$ROOT"/spec/protocols/*.vidl "$tmp/$1/spec/protocols/"
    cp "$ROOT"/v2/lib/vifast-idl/src/generated*.rs "$tmp/$1/v2/lib/vifast-idl/src/"
}

# **KT-420** — bộ giao thức chuẩn + method hợp đồng bắt buộc của từng interface.
# Một dòng/một tệp: `<tên interface>|<danh sách method, cách nhau dấu cách>`.
STANDARD=$'ns|register lookup unregister\nproc|spawn wait kill list\nfs|open read write close mkdir unlink rename readdir stat sync\nblk|capacity sector_size read write flush\nsocket|open connect send recv poll close\nterm|put get'

echo "1..15"
bad=0
case_no=0
ok() { case_no=$((case_no + 1)); echo "ok $case_no - $*"; }
nope() { case_no=$((case_no + 1)); echo "not ok $case_no - $*"; bad=$((bad + 1)); }

expect_fail() { # <tên ca> <dir> — gen --check phải đỏ **vì LỆCH**
    local name="$1" dir="$2"
    if gen "$dir" --check >"$tmp/out" 2>&1; then
        nope "$name KHÔNG bị bắt"
    elif grep -q "LỆCH" "$tmp/out"; then
        ok "$name bị bắt"
    else
        nope "$name bị bắt nhưng sai lý do"; sed 's/^/    /' "$tmp/out"
    fi
}

expect_gen_fail() { # <tên ca> <dir> <idl> <chuỗi lý do mong đợi>
    local name="$1" dir="$2" idl="$3" why="$4"
    if gen "$dir" --idl "$idl" >"$tmp/out" 2>&1; then
        nope "$name: generator CHẤP NHẬN (phải từ chối — $why)"
    elif grep -qF "$why" "$tmp/out"; then
        ok "$name bị từ chối: $why"
    else
        nope "$name bị từ chối nhưng SAI lý do (mong '$why')"; sed 's/^/    /' "$tmp/out"
    fi
}

# ---------------------------------------------------------------- 1..2) gen-check bắt lệch
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

# ---------------------------------------------------------------- 3) đủ 6 tệp chuẩn
missing=""
while IFS='|' read -r name _methods; do
    [ -f "$ROOT/spec/protocols/$name.vidl" ] || missing="$missing $name"
done <<<"$STANDARD"
if [ -z "$missing" ]; then
    ok "đủ 6 .vidl chuẩn trong spec/protocols/ (ns proc fs blk socket term)"
else
    nope "thiếu .vidl chuẩn:$missing"
fi

# ---------------------------------------------------------------- 4) method hợp đồng của từng interface
while IFS='|' read -r name methods; do
    file="$ROOT/spec/protocols/$name.vidl"
    if [ ! -f "$file" ]; then
        nope "$name.vidl: không có tệp để kiểm method"
        continue
    fi
    absent=""
    for m in $methods; do
        grep -qE "^method $m\$" "$file" || absent="$absent $m"
    done
    # Interface chuẩn phải khai đúng tên + version 1 và có `# caps=none` nếu không method nào cap.
    if [ -z "$absent" ] && grep -qE "^interface $name version 1\$" "$file"; then
        ok "$name.vidl: interface $name v1 + đủ method hợp đồng"
    else
        nope "$name.vidl: thiếu method$absent hoặc sai dòng 'interface $name version 1'"
    fi
done <<<"$STANDARD"

# ---------------------------------------------------------------- 5) stub + module cho 5 tệp mới
absent=""
while IFS='|' read -r name _methods; do
    [ "$name" = socket ] && continue   # stub socket đã có từ D2-242 nhóm 6 (6A)
    [ -f "$ROOT/v2/lib/vifast-idl/src/generated_$name.rs" ] || absent="$absent generated_$name.rs"
    grep -qE "^pub mod $name;\$" "$ROOT/v2/lib/vifast-idl/src/lib.rs" || absent="$absent 'pub mod $name;'"
done <<<"$STANDARD"
if [ -z "$absent" ]; then
    ok "stub typed cho 5 interface mới + module trong vifast-idl/lib.rs"
else
    nope "thiếu stub/module:$absent"
fi

# ---------------------------------------------------------------- 6) gen-check trên cây thật
if gen "$ROOT" --check >"$tmp/out" 2>&1; then
    ok "gen-check cây thật: stub khớp mọi .vidl"
else
    nope "gen-check cây thật ĐỎ"; sed 's/^/    /' "$tmp/out"
fi

# ---------------------------------------------------------------- 7..10) ca âm cú pháp mở rộng
mk_tree c
cat >"$tmp/c/spec/protocols/kiem.vidl" <<'EOF'
# caps=none
interface kiem version 1
method mot
  param ten bytes7
  returns u32
EOF
expect_gen_fail "bytes7 (không phải bội 8)" "$tmp/c" "$tmp/c/spec/protocols/kiem.vidl" \
    "bytes phải là bội số của 8"

mk_tree d
cat >"$tmp/d/spec/protocols/kiem.vidl" <<'EOF'
# caps=none
interface kiem version 1
method mot
  param ten bytes8
  reply cap a endpoint
  reply cap b endpoint
  returns u32
EOF
expect_gen_fail "hai 'reply cap' trong một method" "$tmp/d" "$tmp/d/spec/protocols/kiem.vidl" \
    "tối đa MỘT reply cap"

mk_tree e
cat >"$tmp/e/spec/protocols/kiem.vidl" <<'EOF'
interface kiem version 1
method mot
  param ten bytes8
  cap ep khong_ro
  returns u32
EOF
expect_gen_fail "loại cap lạ" "$tmp/e" "$tmp/e/spec/protocols/kiem.vidl" \
    "loại cap 'khong_ro' không hỗ trợ"

mk_tree f
cat >"$tmp/f/spec/protocols/kiem.vidl" <<'EOF'
# caps=none
interface kiem version 1
method mot
  param ten bytes16
  param a u32
  param b u32
  param c u32
  returns u32
EOF
expect_gen_fail "quá 4 MR (bytes16 + 3 scalar = 5)" "$tmp/f" "$tmp/f/spec/protocols/kiem.vidl" \
    "5 MR > 4"

if [ "$case_no" -ne 15 ]; then
    echo "# idl fixtures: SAI SỐ CA ($case_no/15) — sửa dòng '1..15' nếu thêm/bớt ca có chủ ý"
    exit 1
fi
if [ "$bad" -ne 0 ]; then echo "# idl fixtures: FAIL ($bad)"; exit 1; fi
echo "# idl fixtures: PASS"
