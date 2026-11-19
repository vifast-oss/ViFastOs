#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# KT-184 / D2-160 — repro **H4**: `include!` lồng **hai lớp** và **ba lớp** qua tệp test-only.
#
# Dựng lại đúng cách reviewer L16 (KT-174 §2.4) đã phá:
#   - tệp **sản phẩm** `core_sched.rs` có `include!("evil_a.rs")`;
#   - `evil_a.rs` (test-only, `#[cfg(test)] mod`) có `include!("evil_b.rs")`;
#   - `evil_b.rs` (test-only) gọi `m.wake(t)`.
# Bản KT-174 (checker `7e7e1b53`) dùng `resolved` nên **không** lan "sản phẩm thắng" xuống
# lớp thứ hai ⇒ `--strict` vẫn `errors=0`. Bản KT-177 (`eda9dbba`) thay bằng fixpoint ⇒ ĐỎ.
#
# Chạy: bash v2/agent/JOURNAL/kt184-logs/repro-h4.sh
# Ghi:  h4-two-layer.log, h4-three-layer.log (cùng thư mục).
# Không sửa cây thật: mọi đột biến nằm trong thư mục tạm.
set -u

HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../../.." && pwd)
TMP="${TMPDIR:-/tmp}/kt184-h4"
PY=python3
CHECK="$REPO/v2/tools/check-wake-paths.py"

rm -rf "$TMP"
mkdir -p "$TMP"

# Checker cũ: b6efc403 = base của worktree KT-184 (**đã** có fixpoint KT-177);
# 7e7e1b53 = KT-174 (**trước** fixpoint) — đây mới là bản chứng minh lỗ hổng là thật.
git -C "$REPO" show b6efc403:v2/tools/check-wake-paths.py > "$TMP/check-b6efc403.py"
git -C "$REPO" show 7e7e1b53:v2/tools/check-wake-paths.py > "$TMP/check-kt174.py"

build_tree() {
    # $1 = đích; copy đủ `v2/kernel` + `v2/sim` + `v2/lib` cho `include!`/`#[path]`.
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

# ---------------------------------------------------------------- đối chứng sạch
build_tree "$TMP/clean"

# ---------------------------------------------------------------- hai lớp
TWO="$TMP/two"
build_tree "$TWO"
cat > "$TWO/v2/kernel/vifast-core/src/evil_b.rs" <<'EOF'
pub fn evil_nested(m: &mut crate::Machine, t: crate::ObjId) -> Result<(), crate::KError> {
    m.wake(t)
}
EOF
printf 'include!("evil_b.rs");\n' > "$TWO/v2/kernel/vifast-core/src/evil_a.rs"
printf '\n#[cfg(test)]\nmod evil_a;\n#[cfg(test)]\nmod evil_b;\n' \
    >> "$TWO/v2/kernel/vifast-core/src/lib.rs"
printf '\ninclude!("evil_a.rs");\n' >> "$TWO/v2/kernel/vifast-core/src/core_sched.rs"

{
    echo "# KT-184 / D2-160 — H4 hai lớp (core_sched.rs → evil_a.rs → evil_b.rs)"
    echo "# cây: $TWO (bản sao v2/kernel + v2/sim + v2/lib, đột biến trong thư mục tạm)"
    echo
    run "Đối chứng: cây KHÔNG đột biến — checker hiện tại --strict" \
        "$CHECK" "$TMP/clean" --strict
    run "H4 hai lớp — checker HIỆN TẠI (KT-184) --strict" "$CHECK" "$TWO" --strict
    run "H4 hai lớp — checker b6efc403 (base worktree, ĐÃ có fixpoint KT-177) --strict" \
        "$TMP/check-b6efc403.py" "$TWO" --strict
    run "H4 hai lớp — checker 7e7e1b53 (KT-174, TRƯỚC fixpoint KT-177) --strict" \
        "$TMP/check-kt174.py" "$TWO" --strict
} > "$HERE/h4-two-layer.log" 2>&1

# ---------------------------------------------------------------- ba lớp
THREE="$TMP/three"
build_tree "$THREE"
cat > "$THREE/v2/kernel/vifast-core/src/evil_c.rs" <<'EOF'
pub fn evil_deep(m: &mut crate::Machine, t: crate::ObjId) -> Result<(), crate::KError> {
    m.wake(t)
}
EOF
printf 'include!("evil_c.rs");\n' > "$THREE/v2/kernel/vifast-core/src/evil_b.rs"
printf 'include!("evil_b.rs");\n' > "$THREE/v2/kernel/vifast-core/src/evil_a.rs"
printf '\n#[cfg(test)]\nmod evil_a;\n#[cfg(test)]\nmod evil_b;\n#[cfg(test)]\nmod evil_c;\n' \
    >> "$THREE/v2/kernel/vifast-core/src/lib.rs"
printf '\ninclude!("evil_a.rs");\n' >> "$THREE/v2/kernel/vifast-core/src/core_sched.rs"

{
    echo "# KT-184 / D2-160 — H4 BA lớp (core_sched.rs → evil_a.rs → evil_b.rs → evil_c.rs)"
    echo "# cây: $THREE (bản sao v2/kernel + v2/sim + v2/lib, đột biến trong thư mục tạm)"
    echo
    run "H4 ba lớp — checker HIỆN TẠI (KT-184) --strict" "$CHECK" "$THREE" --strict
    run "H4 ba lớp — checker b6efc403 (base worktree, ĐÃ có fixpoint KT-177) --strict" \
        "$TMP/check-b6efc403.py" "$THREE" --strict
    run "H4 ba lớp — checker 7e7e1b53 (KT-174, TRƯỚC fixpoint KT-177) --strict" \
        "$TMP/check-kt174.py" "$THREE" --strict
} > "$HERE/h4-three-layer.log" 2>&1

echo "xong: $HERE/h4-two-layer.log, $HERE/h4-three-layer.log"
