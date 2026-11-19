#!/usr/bin/env bash
# L25 — D2-161 all modes, product + test-only files.
set -u
. "$(cd "$(dirname "$0")" && pwd)/lib.sh"

T=$BASE/d2161

# ---------- product file mutations ----------
build_tree "$T/prod-inc"
cat >> "$T/prod-inc/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'
include!(concat!("evil", "_x.rs"));
EOF
cat > "$T/prod-inc/v2/kernel/vifast-core/src/evil_x.rs" <<'EOF'
impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
EOF

build_tree "$T/prod-path-nonlit"
cat >> "$T/prod-path-nonlit/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'
#[path = concat!("evil", "_x.rs")]
mod evil_x;
EOF
cat > "$T/prod-path-nonlit/v2/kernel/vifast-core/src/evil_x.rs" <<'EOF'
impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
EOF

build_tree "$T/prod-path-out"
cat >> "$T/prod-path-out/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'
#[path = "../../../lib/vifast-abi/src/evil_out.rs"]
mod evil_out;
EOF
cat > "$T/prod-path-out/v2/lib/vifast-abi/src/evil_out.rs" <<'EOF'
impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
EOF

# ---------- test-only file mutations (9921f606: rule applies to ALL files) ----------
build_tree "$T/test-inc"
cat >> "$T/test-inc/v2/kernel/vifast-core/src/lib.rs" <<'EOF'
#[cfg(test)]
mod evil_t;
EOF
cat > "$T/test-inc/v2/kernel/vifast-core/src/evil_t.rs" <<'EOF'
include!(concat!("evil", "_y.rs"));
EOF
cat > "$T/test-inc/v2/kernel/vifast-core/src/evil_y.rs" <<'EOF'
impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
EOF

build_tree "$T/test-path-nonlit"
cat >> "$T/test-path-nonlit/v2/kernel/vifast-core/src/lib.rs" <<'EOF'
#[cfg(test)]
mod evil_t;
EOF
cat > "$T/test-path-nonlit/v2/kernel/vifast-core/src/evil_t.rs" <<'EOF'
#[path = concat!("evil", "_y.rs")]
mod evil_y;
EOF
cat > "$T/test-path-nonlit/v2/kernel/vifast-core/src/evil_y.rs" <<'EOF'
impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
EOF

build_tree "$T/test-path-out"
cat >> "$T/test-path-out/v2/kernel/vifast-core/src/lib.rs" <<'EOF'
#[cfg(test)]
mod evil_t;
EOF
cat > "$T/test-path-out/v2/kernel/vifast-core/src/evil_t.rs" <<'EOF'
#[path = "../../../lib/vifast-abi/src/evil_out.rs"]
mod evil_out;
EOF
cat > "$T/test-path-out/v2/lib/vifast-abi/src/evil_out.rs" <<'EOF'
impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
EOF

for d in prod-inc prod-path-nonlit prod-path-out test-inc test-path-nonlit test-path-out; do
  run "$d / current normal" "$CUR" "$T/$d"
  run "$d / current --strict" "$CUR" "$T/$d" --strict
  run "$d / pre-184 normal" "$PRE" "$T/$d"
done
