#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-189 / L28 — xác nhận nguyên nhân: bộ lọc rẻ đầu vòng vẫn dùng chuỗi CHƯA chuẩn hoá.
set -u
. "$(cd "$(dirname "$0")" && pwd)/lib.sh"
T=$BASE/t4c
OUT='../../../lib/vifast-abi/src/evil_out.rs'
: > "$BASE/logs/t4c.log"
one() {
    label=$1; body=$2
    D=$T/$(echo "$label" | tr -cd 'A-Za-z0-9')
    build_src "$D"
    printf '\n%s\n' "$body" >> "$D/v2/kernel/vifast-core/src/core_sched.rs"
    printf 'pub fn clean() {}\n' > "$D/v2/lib/vifast-abi/src/evil_out.rs"
    out=$("$PY" "$CUR" --root "$D" 2>&1); rc=$?
    echo "### $label" >> "$BASE/logs/t4c.log"
    echo "$out" | grep -E '^E |^# check-wake' >> "$BASE/logs/t4c.log"
    echo "rc=$rc" >> "$BASE/logs/t4c.log"; echo >> "$BASE/logs/t4c.log"
}
# (1) chỉ có dạng cách — bị bỏ qua vì bộ lọc rẻ
one "P1 include ! (…) một mình" "include ! (\"$OUT\");"
# (2) cùng tệp có thêm cfg_attr vô hại ⇒ bộ lọc mở ⇒ regex chuẩn hoá bắt được
one "P2 include ! (…) + cfg_attr vô hại" "include ! (\"$OUT\");
#[cfg_attr(not(test), allow(dead_code))] mod vo_hai;"
# (3) #[ path ] một mình
one "P3 #[ path = … ] một mình" "#[ path = \"$OUT\" ] mod evil_p;"
# (4) #[ path ] + cfg_attr vô hại
one "P4 #[ path = … ] + cfg_attr vô hại" "#[ path = \"$OUT\" ] mod evil_p;
#[cfg_attr(not(test), allow(dead_code))] mod vo_hai;"
# (5) #[\npath] một mình
one "P5 #[\\npath = …] một mình" "#[
path = \"$OUT\"
] mod evil_p;"
# (6) #[\npath] + cfg_attr vô hại
one "P6 #[\\npath = …] + cfg_attr vô hại" "#[
path = \"$OUT\"
] mod evil_p;
#[cfg_attr(not(test), allow(dead_code))] mod vo_hai;"
echo "xong t4c"
