#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-189 / L28 — B1: `#[cfg_attr(not(test), path = "…")]` dựng lại độc lập.
set -u
. "$(cd "$(dirname "$0")" && pwd)/lib.sh"
T=$BASE/b1
EVIL_OUT='use crate::{Machine, ObjId};

impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
'
# ---------------------------------------------------------------- B1 gốc (đúng dạng L25)
D=$T/orig
build_full "$D"
cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

#[cfg_attr(not(test), path = "../../../lib/vifast-abi/src/evil_out.rs")]
mod evil2;
EOF
printf '%s' "$EVIL_OUT" > "$D/v2/lib/vifast-abi/src/evil_out.rs"
{
  echo "# B1 gốc — #[cfg_attr(not(test), path = \"…\")] trỏ ra ngoài cây (v2/lib)"
  echo "# cây: $D"
  echo
  run "B1 CUR (chế độ thường)" "$CUR" "$D"
  run "B1 CUR --strict" "$CUR" "$D" --strict
  run "B1 OLD 8670cdfe (chế độ thường)" "$OLD" "$D"
  run "B1 OLD 8670cdfe --strict" "$OLD" "$D" --strict
  echo "### B1 cargo fmt --all --check"
  ( cd "$D/v2" && cargo fmt --all --check 2>&1 | head -6 ); echo "fmt rc=$?"
  echo "### B1 cargo check -p vifast-core"
  ( cd "$D/v2/kernel" && cargo check -p vifast-core 2>&1 | grep -E 'warning: method|-->|Finished|^error' ); echo "rc=$?"
} > "$BASE/logs/b1-orig.log" 2>&1

# ---------------------------------------------------------------- B1a: cfg_attr(all(not(test), target_os="none"), path)
D=$T/all-cfg
build_full "$D"
cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

#[cfg_attr(all(not(test), target_os = "none"), path = "../../../lib/vifast-abi/src/evil_out.rs")]
mod evil3;
EOF
printf '%s' "$EVIL_OUT" > "$D/v2/lib/vifast-abi/src/evil_out.rs"
{
  echo "# B1a — cfg_attr(all(not(test), target_os=\"none\"), path = \"…\")"
  run "B1a CUR (chế độ thường)" "$CUR" "$D"
  run "B1a CUR --strict" "$CUR" "$D" --strict
  run "B1a OLD 8670cdfe (chế độ thường)" "$OLD" "$D"
  echo "### B1a cargo check -p vifast-core (target_os=none ⇒ path KHÔNG bật trên host)"
  ( cd "$D/v2/kernel" && cargo check -p vifast-core 2>&1 | grep -E 'warning: method|-->|Finished|^error' | head -5 ); echo "rc=$?"
} > "$BASE/logs/b1-allcfg.log" 2>&1

# ---------------------------------------------------------------- B1b: cfg_attr(test, path) — báo oan?
D=$T/test-only-cfg
build_full "$D"
cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

#[cfg(test)]
mod evil_b;
#[cfg_attr(test, path = "evil_b.rs")]
mod evil_b_product;
EOF
cat > "$D/v2/kernel/vifast-core/src/evil_b.rs" <<'EOF'
use crate::{Machine, ObjId};

impl Machine {
    fn chi_test(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
EOF
printf 'pub fn clean() {}\n' > "$D/v2/kernel/vifast-core/src/evil_b_product.rs"
{
  echo "# B1b — cfg_attr(test, path = \"evil_b.rs\") (CHỈ bật khi test) — kiểm báo oan"
  run "B1b CUR (chế độ thường)" "$CUR" "$D"
  run "B1b CUR --strict" "$CUR" "$D" --strict
  run "B1b OLD 8670cdfe --strict" "$OLD" "$D" --strict
  echo "### B1b cargo check -p vifast-core (bản sản phẩm: KHÔNG nạp evil_b.rs)"
  ( cd "$D/v2/kernel" && cargo check -p vifast-core 2>&1 | grep -E 'warning: method|-->|Finished|^error' | head -5 ); echo "rc=$?"
  echo "### B1b cargo check -p vifast-core --tests (bản test: nạp evil_b.rs)"
  ( cd "$D/v2/kernel" && cargo check -p vifast-core --tests 2>&1 | grep -E 'warning: method|-->|Finished|^error' | head -5 ); echo "rc=$?"
} > "$BASE/logs/b1-testcfg.log" 2>&1

# ---------------------------------------------------------------- B1c: cfg_attr(not(test), path = concat!(…))
D=$T/concat
build_full "$D"
cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

#[cfg_attr(not(test), path = concat!("../../../lib/vifast-abi/src/", "evil_out.rs"))]
mod evil4;
EOF
printf '%s' "$EVIL_OUT" > "$D/v2/lib/vifast-abi/src/evil_out.rs"
{
  echo "# B1c — cfg_attr(not(test), path = concat!(…)) — không literal"
  run "B1c CUR (chế độ thường)" "$CUR" "$D"
  run "B1c CUR --strict" "$CUR" "$D" --strict
  run "B1c OLD 8670cdfe (chế độ thường)" "$OLD" "$D"
  echo "### B1c rustc: path = concat!(…) có hợp lệ không?"
  ( cd "$D/v2/kernel" && cargo check -p vifast-core 2>&1 | grep -E '^error|malformed|Finished' | head -5 ); echo "rc=$?"
} > "$BASE/logs/b1-concat.log" 2>&1

echo "xong t1"
