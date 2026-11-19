#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# KT-184 / D2-160 (phần C) — dựng lại **repro H6/H7** của reviewer L16 (KT-174 §2.6/§2.7).
#
#   H6 — `macro_rules! concat` trong cây **che** macro chuẩn cùng tên; thân macro biến ident
#        thành **tên hàm** (`$m.$name($t)`), `wake` là đối số macro "an toàn" + có biến cục bộ
#        `wake` cùng khối ⇒ bản KT-174 miễn ⇒ lọt.
#   H7 — thân `macro_rules!` trong `v2/lib` chứa `include!` (thân **không** có token `wake`);
#        `resolve_include_targets` chỉ lặp trên `v2/kernel`+`v2/sim` nên `include!` trong
#        `v2/lib` không được phân giải ⇒ bản KT-174 xanh.
#
# KT-177 đã đóng cả hai (`_shadowed_safe_macros`, `E_STRICT_MACRO_INCLUDE`). Script này dựng
# lại đột biến trên **bản sao** cây thật để chứng minh checker hiện tại **ĐỎ** — không phải
# chỉ selftest tự khai.
#
# Chạy: bash v2/agent/JOURNAL/kt184-logs/repro-h6h7.sh
# Ghi:  h6.log, h7.log (cùng thư mục).
set -u

HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../../.." && pwd)
TMP="${TMPDIR:-/tmp}/kt184-h6h7"
PY=python3
CHECK="$REPO/v2/tools/check-wake-paths.py"

rm -rf "$TMP"
mkdir -p "$TMP"

# 7e7e1b53 = KT-174 (trước KT-177) — bản còn hở H6/H7.
git -C "$REPO" show 7e7e1b53:v2/tools/check-wake-paths.py > "$TMP/check-kt174.py"

build_tree() {
    rm -rf "$1"
    mkdir -p "$1/v2"
    cp -R "$REPO/v2/kernel" "$1/v2/kernel"
    cp -R "$REPO/v2/sim" "$1/v2/sim"
    cp -R "$REPO/v2/lib" "$1/v2/lib"
}

run() {
    # $1 = nhãn, $2 = checker, $3 = root, $4.. = cờ
    label="$1"
    shift
    checker="$1"
    shift
    root="$1"
    shift
    echo "### $label"
    "$PY" "$checker" --root "$root" "$@" > "$TMP/out.txt" 2>&1
    rc=$?
    grep -E '^E |^# check-wake' "$TMP/out.txt" || true
    echo "rc=$rc"
    echo
}

# ---------------------------------------------------------------- H6
H6="$TMP/h6"
build_tree "$H6"
cat > "$H6/v2/kernel/vifast-core/src/evil_macro.rs" <<'EOF'
macro_rules! concat {
    ($m:expr, $name:ident, $t:expr) => {
        $m.$name($t)
    };
}

pub fn evil_macro(m: &mut crate::Machine, t: crate::ObjId) -> Result<(), crate::KError> {
    let mut wake = 0u32;
    wake += 1;
    concat!(m, wake, t)
}
EOF
printf '\nmod evil_macro;\n' >> "$H6/v2/kernel/vifast-core/src/lib.rs"

{
    echo "# KT-184 / D2-160 (C) — H6: macro_rules! concat che tên an toàn + biến cục bộ wake"
    echo "# cây: $H6 (bản sao v2/kernel + v2/sim + v2/lib, đột biến trong thư mục tạm)"
    echo "# KT-177 §2.3 đã kiểm đột biến này biên dịch được: cargo check -p vifast-core RC=0."
    echo
    run "H6 — checker HIỆN TẠI (KT-184) --strict" "$CHECK" "$H6" --strict
    run "H6 — checker 7e7e1b53 (KT-174, TRƯỚC KT-177) --strict" \
        "$TMP/check-kt174.py" "$H6" --strict
} > "$HERE/h6.log" 2>&1

# ---------------------------------------------------------------- H7
H7="$TMP/h7"
build_tree "$H7"
cat > "$H7/v2/lib/vifast-abi/src/evil_inc_body.rs" <<'EOF'
pub fn evil_inc_body(m: &mut crate::Machine, t: crate::ObjId) -> Result<(), crate::KError> {
    m.wake(t)
}
EOF
cat >> "$H7/v2/lib/vifast-abi/src/lib.rs" <<'EOF'

#[macro_export]
macro_rules! evil_inc {
    () => {
        include!("../../../lib/vifast-abi/src/evil_inc_body.rs")
    };
}
EOF
cat >> "$H7/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'

pub fn evil_inc_probe(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    vifast_abi::evil_inc!();
    Ok(())
}
EOF

{
    echo "# KT-184 / D2-160 (C) — H7: thân macro_rules! trong v2/lib chứa include!"
    echo "# cây: $H7 (bản sao v2/kernel + v2/sim + v2/lib, đột biến trong thư mục tạm)"
    echo "# KT-177 §2.4 đã kiểm đột biến này biên dịch được: cargo check -p vifast-core RC=0."
    echo
    run "H7 — checker HIỆN TẠI (KT-184) --strict" "$CHECK" "$H7" --strict
    run "H7 — checker 7e7e1b53 (KT-174, TRƯỚC KT-177) --strict" \
        "$TMP/check-kt174.py" "$H7" --strict
} > "$HERE/h7.log" 2>&1

echo "xong: $HERE/h6.log, $HERE/h7.log"
