#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-189 / L28 — T4b: dạng viết cách khác, đích RA NGOÀI cây (phải ĐỎ nếu bắt được).
set -u
. "$(cd "$(dirname "$0")" && pwd)/lib.sh"
T=$BASE/t4b
OUT='../../../lib/vifast-abi/src/evil_out.rs'
: > "$BASE/logs/t4b.log"
one() {
    label=$1; line=$2
    D=$T/$(echo "$label" | tr -cd 'A-Za-z0-9')
    build_src "$D"
    printf '\n%s\n' "$line" >> "$D/v2/kernel/vifast-core/src/core_sched.rs"
    printf 'pub fn clean() {}\n' > "$D/v2/lib/vifast-abi/src/evil_out.rs"
    out=$("$PY" "$CUR" --root "$D" 2>&1); rc=$?
    echo "### $label" >> "$BASE/logs/t4b.log"
    echo "$out" | grep -E '^E |^# check-wake' >> "$BASE/logs/t4b.log"
    echo "rc=$rc" >> "$BASE/logs/t4b.log"; echo >> "$BASE/logs/t4b.log"
}
one "T4a include ! (…)"       "include ! (\"$OUT\");"
one "T4b include!{…}"         "include!{\"$OUT\"};"
one "T4c include![…]"         "include![\"$OUT\"];"
one "T4d include ! { … }"     "include ! { \"$OUT\" };"
one "T4e include!(r\"…\")"     "include!(r\"$OUT\");"
one "T4f include!(r#\"…\"#)"   "include!(r#\"$OUT\"#);"
one "T4g include! /*c*/ (…)"  "include! /*c*/ (\"$OUT\");"
one "T4i #[ path = … ]"       "#[ path = \"$OUT\" ] mod evil_p;"
one "T4j #[path=\"…\"]"       "#[path=\"$OUT\"] mod evil_p;"
one "T4k # [ path = … ]"      "# [ path = \"$OUT\" ] mod evil_p;"
one "T4l #[path = r\"…\"]"     "#[path = r\"$OUT\"] mod evil_p;"
one "T4n cfg_attr không cách" "#[cfg_attr(not(test),path=\"$OUT\")] mod evil_p;"
one "T4o cfg_attr r\"…\""      "#[cfg_attr(not(test), path = r\"$OUT\")] mod evil_p;"
one "T4p cfg_attr nhiều attr" "#[cfg_attr(not(test), path = \"$OUT\", allow(dead_code))] mod evil_p;"
one "T4q cfg_attr lồng"       "#[cfg_attr(not(test), cfg_attr(unix, path = \"$OUT\"))] mod evil_p;"
one "T4s cfg_attr xuống dòng" "#[cfg_attr(not(test),
    path = \"$OUT\")] mod evil_p;"
echo "xong t4b"
