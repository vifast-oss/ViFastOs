#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# KT-184 / D2-161 — luật cứng `include!`/`#[path]` ở **MỌI chế độ**.
#
# Ba đột biến bắt buộc, chạy ở **chế độ thường** (KHÔNG `--strict`):
#   B1. `include!(concat!("evil", "_b.rs"))` trong mã sản phẩm;
#   B2. `#[path = r#"../../../lib/vifast-abi/src/evil_out.rs"#]` trỏ **ra ngoài** cây;
#   B3. `#[path = concat!("evil", ".rs")]` **không literal**.
#
# Trước KT-184 khối phân giải nằm **trong** `if strict:` nên `v2 check` thường (và `gate`)
# trả `errors=0` cho cả ba. B3 còn lọt cả `--strict` (chỉ `include!` không literal bị bắt).
#
# Chạy: bash v2/agent/JOURNAL/kt184-logs/repro-d2161.sh
# Ghi:  d2161-normal.log (chế độ thường), d2161-strict.log (đối chứng --strict).
set -u

HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../../.." && pwd)
TMP="${TMPDIR:-/tmp}/kt184-d2161"
PY=python3
CHECK="$REPO/v2/tools/check-wake-paths.py"

rm -rf "$TMP"
mkdir -p "$TMP"

# b6efc403 = base của worktree KT-184 (trước bản vá D2-161 của KT-184).
git -C "$REPO" show b6efc403:v2/tools/check-wake-paths.py > "$TMP/check-b6efc403.py"

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

# ---------------------------------------------------------------- B1
B1="$TMP/b1"
build_tree "$B1"
cat > "$B1/v2/kernel/vifast-core/src/evil_b.rs" <<'EOF'
pub fn evil_nested(m: &mut crate::Machine, t: crate::ObjId) -> Result<(), crate::KError> {
    m.wake(t)
}
EOF
printf '\ninclude!(concat!("evil", "_b.rs"));\n' \
    >> "$B1/v2/kernel/vifast-core/src/core_sched.rs"

# ---------------------------------------------------------------- B2
B2="$TMP/b2"
build_tree "$B2"
cat > "$B2/v2/lib/vifast-abi/src/evil_out.rs" <<'EOF'
pub fn evil_out(m: &mut crate::Machine, t: crate::ObjId) -> Result<(), crate::KError> {
    m.wake(t)
}
EOF
printf '\n#[path = r#"../../../lib/vifast-abi/src/evil_out.rs"#]\nmod evil_out;\n' \
    >> "$B2/v2/kernel/vifast-core/src/core_sched.rs"

# ---------------------------------------------------------------- B3
B3="$TMP/b3"
build_tree "$B3"
cat > "$B3/v2/kernel/vifast-core/src/evil_x.rs" <<'EOF'
pub fn evil_x(m: &mut crate::Machine, t: crate::ObjId) -> Result<(), crate::KError> {
    m.wake(t)
}
EOF
printf '\n#[path = concat!("evil", "_x.rs")]\nmod evil_x;\n' \
    >> "$B3/v2/kernel/vifast-core/src/core_sched.rs"

{
    echo "# KT-184 / D2-161 — ba đột biến ở CHẾ ĐỘ THƯỜNG (không --strict)"
    echo "# cây: $TMP (bản sao v2/kernel + v2/sim + v2/lib, đột biến trong thư mục tạm)"
    echo
    run "B1 include!(concat!(…)) — checker HIỆN TẠI (KT-184), chế độ thường" \
        "$CHECK" "$B1"
    run "B1 include!(concat!(…)) — checker b6efc403 (trước KT-184), chế độ thường" \
        "$TMP/check-b6efc403.py" "$B1"
    run "B2 #[path] ra ngoài cây — checker HIỆN TẠI (KT-184), chế độ thường" \
        "$CHECK" "$B2"
    run "B2 #[path] ra ngoài cây — checker b6efc403 (trước KT-184), chế độ thường" \
        "$TMP/check-b6efc403.py" "$B2"
    run "B3 #[path = concat!(…)] — checker HIỆN TẠI (KT-184), chế độ thường" \
        "$CHECK" "$B3"
    run "B3 #[path = concat!(…)] — checker b6efc403 (trước KT-184), chế độ thường" \
        "$TMP/check-b6efc403.py" "$B3"
} > "$HERE/d2161-normal.log" 2>&1

{
    echo "# KT-184 / D2-161 — đối chứng: cùng ba đột biến ở --strict"
    echo "# (B3 lọt cả --strict trước KT-184: chỉ include! không literal bị bắt)"
    echo
    run "B1 include!(concat!(…)) — checker b6efc403, --strict" \
        "$TMP/check-b6efc403.py" "$B1" --strict
    run "B2 #[path] ra ngoài cây — checker b6efc403, --strict" \
        "$TMP/check-b6efc403.py" "$B2" --strict
    run "B3 #[path = concat!(…)] — checker b6efc403, --strict" \
        "$TMP/check-b6efc403.py" "$B3" --strict
    run "B3 #[path = concat!(…)] — checker HIỆN TẠI (KT-184), --strict" \
        "$CHECK" "$B3" --strict
} > "$HERE/d2161-strict.log" 2>&1

echo "xong: $HERE/d2161-normal.log, $HERE/d2161-strict.log"
