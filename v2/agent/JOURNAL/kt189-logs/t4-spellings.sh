#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-189 / L28 — T4: dạng viết cách khác; T5: báo oan.
set -u
. "$(cd "$(dirname "$0")" && pwd)/lib.sh"
T=$BASE/t4
EVIL='use crate::{Machine, ObjId};

impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
'
: > "$BASE/logs/t4.log"

# Mỗi ca: cây nguồn, thêm một dòng vào core_sched.rs (sản phẩm), đích test-only có wake.
one() {
    label=$1; line=$2; mode=$3
    D=$T/$(echo "$label" | tr -cd 'A-Za-z0-9')
    build_src "$D"
    printf '\n%s\n' "$line" >> "$D/v2/kernel/vifast-core/src/core_sched.rs"
    cat > "$D/v2/kernel/vifast-core/src/evil_t.rs" <<'EOF'
use crate::{Machine, ObjId};

impl Machine {
    fn chi_test(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
EOF
    # khai báo test-only để tệp đích là test-only
    printf '\n#[cfg(test)]\nmod evil_t;\n' >> "$D/v2/kernel/vifast-core/src/lib.rs"
    out=$("$PY" "$CUR" --root "$D" $mode 2>&1); rc=$?
    echo "### $label [$mode]" >> "$BASE/logs/t4.log"
    echo "$out" | grep -E '^E |^# check-wake' >> "$BASE/logs/t4.log"
    echo "rc=$rc" >> "$BASE/logs/t4.log"
    echo >> "$BASE/logs/t4.log"
}

# --- T4: include! dạng viết cách khác (đích test-only có wake) ---
one "T4a include ! (…)"            'include ! ("evil_t.rs");' ""
one "T4b include!{…}"              'include!{"evil_t.rs"};' ""
one "T4c include![…]"              'include!["evil_t.rs"];' ""
one "T4d include ! { … }"          'include ! { "evil_t.rs" };' ""
one "T4e include!(r\"…\")"          'include!(r"evil_t.rs");' ""
one "T4f include!(r#\"…\"#)"        'include!(r#"evil_t.rs"#);' ""
one "T4g include! /*c*/ (…)"       'include! /*c*/ ("evil_t.rs");' ""
one "T4h include!(concat!(…))"     'include!(concat!("evil", "_t.rs"));' ""

# --- T4: #[path] dạng viết cách khác ---
one "T4i #[ path = … ]"            '#[ path = "evil_t.rs" ] mod evil_p;' ""
one "T4j #[path=\"…\"]"            '#[path="evil_t.rs"] mod evil_p;' ""
one "T4k # [ path = … ]"           '# [ path = "evil_t.rs" ] mod evil_p;' ""
one "T4l #[path = r\"…\"]"          '#[path = r"evil_t.rs"] mod evil_p;' ""
one "T4m #[path = concat!(…)]"     '#[path = concat!("evil", "_t.rs")] mod evil_p;' ""

# --- T4: cfg_attr(path) dạng viết cách khác ---
one "T4n cfg_attr không dấu cách"  '#[cfg_attr(not(test),path="evil_t.rs")] mod evil_p;' ""
one "T4o cfg_attr path r\"…\""      '#[cfg_attr(not(test), path = r"evil_t.rs")] mod evil_p;' ""
one "T4p cfg_attr nhiều attr"       '#[cfg_attr(not(test), path = "evil_t.rs", allow(dead_code))] mod evil_p;' ""
one "T4q cfg_attr lồng"             '#[cfg_attr(not(test), cfg_attr(unix, path = "evil_t.rs"))] mod evil_p;' ""
one "T4r cfg_attr path concat!"     '#[cfg_attr(not(test), path = concat!("evil", "_t.rs"))] mod evil_p;' ""
one "T4s cfg_attr xuống dòng"       '#[cfg_attr(not(test),
    path = "evil_t.rs")] mod evil_p;' ""

# --- T5: báo oan ---
# T5a: cfg_attr(test, path) — chỉ bật khi test; bản sản phẩm KHÔNG nạp đích
D=$T/T5a
build_src "$D"
cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

#[cfg_attr(test, path = "evil_t.rs")]
mod evil_p;
EOF
printf 'pub fn clean() {}\n' > "$D/v2/kernel/vifast-core/src/evil_p.rs"
printf '%s' "$EVIL" > "$D/v2/kernel/vifast-core/src/evil_t.rs"
{
  echo "### T5a cfg_attr(test, path = \"evil_t.rs\") — CHỈ bật khi test (đích có wake)"
  run "T5a CUR (chế độ thường)" "$CUR" "$D"
  run "T5a CUR --strict" "$CUR" "$D" --strict
  run "T5a OLD 8670cdfe --strict" "$OLD" "$D" --strict
} >> "$BASE/logs/t4.log" 2>&1

# T5b: include! trong #[cfg(test)] của tệp sản phẩm, đích trong cây có wake
D=$T/T5b
build_src "$D"
cat >> "$D/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'

#[cfg(test)]
mod chi_test {
    include!("evil_t.rs");
}
EOF
printf '%s' "$EVIL" > "$D/v2/kernel/vifast-core/src/evil_t.rs"
{
  echo "### T5b include! trong #[cfg(test)] của tệp SẢN PHẨM, đích trong cây có wake"
  run "T5b CUR (chế độ thường)" "$CUR" "$D"
  run "T5b CUR --strict" "$CUR" "$D" --strict
  run "T5b OLD 8670cdfe --strict" "$OLD" "$D" --strict
} >> "$BASE/logs/t4.log" 2>&1

# T5c: #[path] trong #[cfg(test)] của tệp sản phẩm, đích trong cây có wake
D=$T/T5c
build_src "$D"
cat >> "$D/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'

#[cfg(test)]
#[path = "evil_t.rs"]
mod chi_test2;
EOF
printf '%s' "$EVIL" > "$D/v2/kernel/vifast-core/src/evil_t.rs"
{
  echo "### T5c #[path] trong #[cfg(test)] của tệp SẢN PHẨM, đích trong cây có wake"
  run "T5c CUR (chế độ thường)" "$CUR" "$D"
  run "T5c CUR --strict" "$CUR" "$D" --strict
  run "T5c OLD 8670cdfe --strict" "$OLD" "$D" --strict
} >> "$BASE/logs/t4.log" 2>&1

echo "xong t4"
