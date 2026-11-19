#!/usr/bin/env bash
# L25 — formatting-based bypasses: whitespace in include!/#[path], cfg_attr path.
set -u
. "$(cd "$(dirname "$0")" && pwd)/lib.sh"

T=$BASE/fmt
W='use crate::{Machine, ObjId};

impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
'

# ---------- include ! (space before bang) ----------
build_tree "$T/inc-space"
cat >> "$T/inc-space/v2/kernel/vifast-core/src/lib.rs" <<'EOF'
#[cfg(test)]
mod evil;
EOF
printf '%s' "$W" > "$T/inc-space/v2/kernel/vifast-core/src/evil.rs"
cat >> "$T/inc-space/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'
include ! ("evil.rs");
EOF

# ---------- #[ path = ... ] (space after #[) ----------
build_tree "$T/path-space"
cat >> "$T/path-space/v2/kernel/vifast-core/src/lib.rs" <<'EOF'
#[cfg(test)]
mod evil;
#[ path = "evil.rs" ]
mod evil2;
EOF
printf '%s' "$W" > "$T/path-space/v2/kernel/vifast-core/src/evil.rs"

# ---------- #[\npath = ...\n] (newline) ----------
build_tree "$T/path-newline"
cat >> "$T/path-newline/v2/kernel/vifast-core/src/lib.rs" <<'EOF'
#[cfg(test)]
mod evil;
#[
path = "evil.rs"
]
mod evil2;
EOF
printf '%s' "$W" > "$T/path-newline/v2/kernel/vifast-core/src/evil.rs"

# ---------- cfg_attr path outside tree ----------
build_tree "$T/cfgattr-out"
cat >> "$T/cfgattr-out/v2/kernel/vifast-core/src/lib.rs" <<'EOF'
#[cfg(test)]
mod evil;
#[cfg_attr(not(test), path = "../../../lib/vifast-abi/src/evil_out.rs")]
mod evil2;
EOF
printf '%s' "$W" > "$T/cfgattr-out/v2/lib/vifast-abi/src/evil_out.rs"

for d in inc-space path-space path-newline cfgattr-out; do
  run "$d / current normal" "$CUR" "$T/$d"
  run "$d / current --strict" "$CUR" "$T/$d" --strict
done
