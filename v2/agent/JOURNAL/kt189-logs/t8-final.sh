#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-189 / L28 — T8: macro ở v2/sim, macro v2/lib + cfg_attr, báo oan cfg_attr(test).
set -u
. "$(cd "$(dirname "$0")" && pwd)/lib.sh"
T=$BASE/t8
EVIL='use crate::{Machine, ObjId};

impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
'
: > "$BASE/logs/t8.log"
probe() {
    label=$1; D=$2
    {
      echo "### $label"
      run "$label CUR (chế độ thường)" "$CUR" "$D"
      run "$label CUR --strict" "$CUR" "$D" --strict
      echo "### $label cargo fmt --all --check"
      ( cd "$D/v2" && cargo fmt --all --check >/dev/null 2>&1; echo "fmt rc=$?" )
      echo "### $label cargo check -p vifast-core"
      ( cd "$D/v2/kernel" && cargo check -p vifast-core 2>&1 | grep -E 'warning: method|-->|Finished|^error' | head -4 )
      echo
    } >> "$BASE/logs/t8.log" 2>&1
}

# (1) macro ở v2/sim, #[path] ngoặc nhọn, gọi từ v2/kernel
D=$T/sim-macro
build_full "$D"
cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

mod sub;
EOF
cat > "$D/v2/sim/vifast-sim/src/m.rs" <<'EOF'
#[macro_export]
macro_rules! evil_sim_mod {
    () => { #[path = "evil.rs"] mod x; };
}
EOF
printf 'pub fn clean() {}\n' > "$D/v2/sim/vifast-sim/src/evil.rs"
cat > "$D/v2/kernel/vifast-core/src/sub.rs" <<'EOF'
#[cfg(test)]
mod evil;
mod s;
EOF
mkdir -p "$D/v2/kernel/vifast-core/src/sub"
printf 'vifast_sim::evil_sim_mod!();\n' > "$D/v2/kernel/vifast-core/src/sub/s.rs"
printf '%s' "$EVIL" > "$D/v2/kernel/vifast-core/src/sub/evil.rs"
probe "T8a macro ở v2/sim + #[path] ngoặc nhọn, gọi từ v2/kernel" "$D"

# (2) macro ở v2/sim, #[path] ngoặc TRÒN
D=$T/sim-macro-paren
build_full "$D"
cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

mod sub;
EOF
cat > "$D/v2/sim/vifast-sim/src/m.rs" <<'EOF'
#[macro_export]
macro_rules! evil_sim_paren (
    () => (
        #[path = "evil.rs"] mod x;
    );
);
EOF
printf 'pub fn clean() {}\n' > "$D/v2/sim/vifast-sim/src/evil.rs"
cat > "$D/v2/kernel/vifast-core/src/sub.rs" <<'EOF'
#[cfg(test)]
mod evil;
mod s;
EOF
mkdir -p "$D/v2/kernel/vifast-core/src/sub"
printf 'vifast_sim::evil_sim_paren!();\n' > "$D/v2/kernel/vifast-core/src/sub/s.rs"
printf '%s' "$EVIL" > "$D/v2/kernel/vifast-core/src/sub/evil.rs"
probe "T8b macro ở v2/sim + #[path] ngoặc TRÒN" "$D"

# (3) macro ở v2/lib + cfg_attr(path), gọi từ v2/kernel
D=$T/lib-cfgattr
build_full "$D"
cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

mod sub;
EOF
cat > "$D/v2/lib/vifast-abi/src/m.rs" <<'EOF'
#[macro_export]
macro_rules! evil_lib_cfg {
    () => { #[cfg_attr(not(test), path = "evil.rs")] mod x; };
}
EOF
printf 'pub fn clean() {}\n' > "$D/v2/lib/vifast-abi/src/evil.rs"
cat > "$D/v2/kernel/vifast-core/src/sub.rs" <<'EOF'
#[cfg(test)]
mod evil;
mod s;
EOF
mkdir -p "$D/v2/kernel/vifast-core/src/sub"
printf 'vifast_abi::evil_lib_cfg!();\n' > "$D/v2/kernel/vifast-core/src/sub/s.rs"
printf '%s' "$EVIL" > "$D/v2/kernel/vifast-core/src/sub/evil.rs"
probe "T8c macro ở v2/lib + cfg_attr(path), gọi từ v2/kernel" "$D"

# (4) báo oan: cfg_attr(test, path) trỏ tệp test-only ĐÃ khai báo ở chỗ khác
D=$T/fp-cfgattr
build_full "$D"
cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

#[cfg(test)]
#[path = "evil_t.rs"]
mod evil_t;
#[cfg_attr(test, path = "evil_t.rs")]
mod evil_p;
EOF
printf 'pub fn clean() {}\n' > "$D/v2/kernel/vifast-core/src/evil_p.rs"
cat > "$D/v2/kernel/vifast-core/src/evil_t.rs" <<'EOF'
pub fn chi_test(m: &mut crate::Machine, t: crate::ObjId) {
    let _ = m.wake(t);
}
EOF
{
  echo "### T8d báo oan: cfg_attr(test, path) trỏ tệp test-only đã khai báo (bản sản phẩm KHÔNG nạp)"
  run "T8d CUR (chế độ thường)" "$CUR" "$D"
  run "T8d CUR --strict" "$CUR" "$D" --strict
  run "T8d OLD 8670cdfe --strict" "$OLD" "$D" --strict
  echo "### T8d cargo check -p vifast-core (sản phẩm)"
  ( cd "$D/v2/kernel" && cargo check -p vifast-core 2>&1 | grep -E 'warning: method|-->|Finished|^error' | head -4 )
  echo "### T8d cargo check -p vifast-core --tests (test)"
  ( cd "$D/v2/kernel" && cargo check -p vifast-core --tests 2>&1 | grep -E 'warning: method|-->|Finished|^error' | head -4 )
  echo
} >> "$BASE/logs/t8.log" 2>&1

echo "xong t8"
