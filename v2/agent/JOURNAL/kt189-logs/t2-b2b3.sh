#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-189 / L28 — B2/B3: #[path]/include! trong thân macro_rules!, dựng lại độc lập.
set -u
. "$(cd "$(dirname "$0")" && pwd)/lib.sh"
T=$BASE/b2
EVIL='use crate::{Machine, ObjId};

impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
'

# Khung chung: macro định nghĩa ở src/m.rs (decoy sạch src/evil.rs), gọi ở src/sub/s.rs
# (tệp thật src/sub/evil.rs test-only, có wake).
mk_macro_tree() {
    D=$1; BODY=$2
    build_full "$D"
    cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

mod m;
mod sub;
EOF
    cat > "$D/v2/kernel/vifast-core/src/m.rs" <<EOF
#[macro_export]
macro_rules! evil_mod {
    () => { $BODY };
}
EOF
    printf 'pub fn clean() {}\n' > "$D/v2/kernel/vifast-core/src/evil.rs"
    cat > "$D/v2/kernel/vifast-core/src/sub.rs" <<'EOF'
#[cfg(test)]
mod evil;
mod s;
EOF
    mkdir -p "$D/v2/kernel/vifast-core/src/sub"
    printf 'crate::evil_mod!();\n' > "$D/v2/kernel/vifast-core/src/sub/s.rs"
    printf '%s' "$EVIL" > "$D/v2/kernel/vifast-core/src/sub/evil.rs"
}

# ---------------------------------------------------------------- B2 gốc
D=$T/path
mk_macro_tree "$D" '#[path = "evil.rs"] mod x;'
{
  echo "# B2 gốc — #[path] trong thân macro_rules! (rustc giải theo tệp gọi)"
  run "B2 CUR (chế độ thường)" "$CUR" "$D"
  run "B2 CUR --strict" "$CUR" "$D" --strict
  run "B2 OLD 8670cdfe (chế độ thường)" "$OLD" "$D"
  run "B2 OLD 8670cdfe --strict" "$OLD" "$D" --strict
  echo "### B2 cargo check -p vifast-core"
  ( cd "$D/v2/kernel" && cargo check -p vifast-core 2>&1 | grep -E 'warning: method|-->|Finished|^error' | head -5 ); echo "rc=$?"
} > "$BASE/logs/b2-orig.log" 2>&1

# ---------------------------------------------------------------- B3 gốc
D=$T/inc
mk_macro_tree "$D" 'include!("evil.rs");'
{
  echo "# B3 gốc — include! trong thân macro_rules! (rustc giải theo tệp gọi)"
  run "B3 CUR (chế độ thường)" "$CUR" "$D"
  run "B3 CUR --strict" "$CUR" "$D" --strict
  run "B3 OLD 8670cdfe (chế độ thường)" "$OLD" "$D"
  run "B3 OLD 8670cdfe --strict" "$OLD" "$D" --strict
  echo "### B3 cargo check -p vifast-core"
  ( cd "$D/v2/kernel" && cargo check -p vifast-core 2>&1 | grep -E 'warning: method|-->|Finished|^error' | head -5 ); echo "rc=$?"
} > "$BASE/logs/b3-orig.log" 2>&1

# ---------------------------------------------------------------- B2a: macro định nghĩa ở v2/lib, gọi ở v2/kernel
D=$T/lib-macro
build_full "$D"
cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

mod sub;
EOF
cat > "$D/v2/lib/vifast-abi/src/m.rs" <<'EOF'
#[macro_export]
macro_rules! evil_lib_mod {
    () => { #[path = "evil.rs"] mod x; };
}
EOF
cat > "$D/v2/kernel/vifast-core/src/sub.rs" <<'EOF'
#[cfg(test)]
mod evil;
mod s;
EOF
mkdir -p "$D/v2/kernel/vifast-core/src/sub"
printf 'vifast_abi::evil_lib_mod!();\n' > "$D/v2/kernel/vifast-core/src/sub/s.rs"
printf '%s' "$EVIL" > "$D/v2/kernel/vifast-core/src/sub/evil.rs"
{
  echo "# B2a — macro định nghĩa ở v2/lib, gọi ở v2/kernel"
  run "B2a CUR (chế độ thường)" "$CUR" "$D"
  run "B2a CUR --strict" "$CUR" "$D" --strict
  run "B2a OLD 8670cdfe (chế độ thường)" "$OLD" "$D"
} > "$BASE/logs/b2-libmacro.log" 2>&1

# ---------------------------------------------------------------- B2b: macro lồng trong macro
D=$T/nested
mk_macro_tree "$D" 'macro_rules! inner { () => { #[path = "evil.rs"] mod x; }; } inner!();'
{
  echo "# B2b — macro lồng trong macro (định nghĩa + gọi trong thân macro ngoài)"
  run "B2b CUR (chế độ thường)" "$CUR" "$D"
  run "B2b CUR --strict" "$CUR" "$D" --strict
  run "B2b OLD 8670cdfe (chế độ thường)" "$OLD" "$D"
  echo "### B2b cargo check -p vifast-core"
  ( cd "$D/v2/kernel" && cargo check -p vifast-core 2>&1 | grep -E 'warning: method|-->|Finished|^error' | head -5 ); echo "rc=$?"
} > "$BASE/logs/b2-nested.log" 2>&1

# ---------------------------------------------------------------- B2c: macro_rules! dùng ngoặc tròn
D=$T/paren
build_full "$D"
cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

mod m;
mod sub;
EOF
cat > "$D/v2/kernel/vifast-core/src/m.rs" <<'EOF'
#[macro_export]
macro_rules! evil_paren (
    () => (
        #[path = "evil.rs"] mod x;
    );
);
EOF
printf 'pub fn clean() {}\n' > "$D/v2/kernel/vifast-core/src/evil.rs"
cat > "$D/v2/kernel/vifast-core/src/sub.rs" <<'EOF'
#[cfg(test)]
mod evil;
mod s;
EOF
mkdir -p "$D/v2/kernel/vifast-core/src/sub"
printf 'crate::evil_paren!();\n' > "$D/v2/kernel/vifast-core/src/sub/s.rs"
printf '%s' "$EVIL" > "$D/v2/kernel/vifast-core/src/sub/evil.rs"
{
  echo "# B2c — macro_rules! dùng ngoặc TRÒN: macro_rules! evil_paren ( () => ( #[path] mod x; ); );"
  run "B2c CUR (chế độ thường)" "$CUR" "$D"
  run "B2c CUR --strict" "$CUR" "$D" --strict
  run "B2c OLD 8670cdfe (chế độ thường)" "$OLD" "$D"
  echo "### B2c cargo check -p vifast-core"
  ( cd "$D/v2/kernel" && cargo check -p vifast-core 2>&1 | grep -E 'warning: method|-->|Finished|^error' | head -5 ); echo "rc=$?"
} > "$BASE/logs/b2-paren.log" 2>&1

# ---------------------------------------------------------------- B2d: cfg_attr(path) trong thân macro
D=$T/cfgattr-macro
mk_macro_tree "$D" '#[cfg_attr(not(test), path = "evil.rs")] mod x;'
{
  echo "# B2d — #[cfg_attr(not(test), path = \"…\")] TRONG thân macro_rules! (B1+B2)"
  run "B2d CUR (chế độ thường)" "$CUR" "$D"
  run "B2d CUR --strict" "$CUR" "$D" --strict
  run "B2d OLD 8670cdfe (chế độ thường)" "$OLD" "$D"
  echo "### B2d cargo check -p vifast-core"
  ( cd "$D/v2/kernel" && cargo check -p vifast-core 2>&1 | grep -E 'warning: method|-->|Finished|^error' | head -5 ); echo "rc=$?"
} > "$BASE/logs/b2-cfgattr-macro.log" 2>&1

# ---------------------------------------------------------------- B3a: include! trong macro ngoặc tròn
D=$T/inc-paren
build_full "$D"
cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

mod m;
mod sub;
EOF
cat > "$D/v2/kernel/vifast-core/src/m.rs" <<'EOF'
#[macro_export]
macro_rules! evil_inc_paren (
    () => (
        include!("evil.rs");
    );
);
EOF
printf 'pub fn clean() {}\n' > "$D/v2/kernel/vifast-core/src/evil.rs"
cat > "$D/v2/kernel/vifast-core/src/sub.rs" <<'EOF'
#[cfg(test)]
mod evil;
mod s;
EOF
mkdir -p "$D/v2/kernel/vifast-core/src/sub"
printf 'crate::evil_inc_paren!();\n' > "$D/v2/kernel/vifast-core/src/sub/s.rs"
printf '%s' "$EVIL" > "$D/v2/kernel/vifast-core/src/sub/evil.rs"
{
  echo "# B3a — include! trong macro_rules! ngoặc TRÒN"
  run "B3a CUR (chế độ thường)" "$CUR" "$D"
  run "B3a CUR --strict" "$CUR" "$D" --strict
  run "B3a OLD 8670cdfe (chế độ thường)" "$OLD" "$D"
  echo "### B3a cargo check -p vifast-core"
  ( cd "$D/v2/kernel" && cargo check -p vifast-core 2>&1 | grep -E 'warning: method|-->|Finished|^error' | head -5 ); echo "rc=$?"
} > "$BASE/logs/b3-paren.log" 2>&1

echo "xong t2"
