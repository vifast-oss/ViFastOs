#!/usr/bin/env bash
# L25 — H4 two/three layer, independent rebuild.
set -u
. "$(cd "$(dirname "$0")" && pwd)/lib.sh"

T=$BASE/h4
build_tree "$T/clean"

# ---------- two layers ----------
build_tree "$T/two"
cat >> "$T/two/v2/kernel/vifast-core/src/lib.rs" <<'EOF'
#[cfg(test)]
mod evil_a;
#[cfg(test)]
mod evil_b;
EOF
cat > "$T/two/v2/kernel/vifast-core/src/evil_a.rs" <<'EOF'
include!("evil_b.rs");
EOF
cat > "$T/two/v2/kernel/vifast-core/src/evil_b.rs" <<'EOF'
impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
EOF
cat >> "$T/two/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'
include!("evil_a.rs");
EOF

# ---------- three layers ----------
build_tree "$T/three"
cat >> "$T/three/v2/kernel/vifast-core/src/lib.rs" <<'EOF'
#[cfg(test)]
mod evil_a;
#[cfg(test)]
mod evil_b;
#[cfg(test)]
mod evil_c;
EOF
cat > "$T/three/v2/kernel/vifast-core/src/evil_a.rs" <<'EOF'
include!("evil_b.rs");
EOF
cat > "$T/three/v2/kernel/vifast-core/src/evil_b.rs" <<'EOF'
include!("evil_c.rs");
EOF
cat > "$T/three/v2/kernel/vifast-core/src/evil_c.rs" <<'EOF'
impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
EOF
cat >> "$T/three/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'
include!("evil_a.rs");
EOF

run "clean tree / current --strict" "$CUR" "$T/clean" --strict
run "two layers / current --strict" "$CUR" "$T/two" --strict
run "two layers / current normal" "$CUR" "$T/two"
run "two layers / pre-184 (9921f606^) --strict" "$PRE" "$T/two" --strict
run "two layers / 7e7e1b53 --strict" "$OLD" "$T/two" --strict
run "three layers / current --strict" "$CUR" "$T/three" --strict
run "three layers / 7e7e1b53 --strict" "$OLD" "$T/three" --strict
