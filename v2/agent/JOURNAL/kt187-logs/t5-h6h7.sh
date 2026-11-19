#!/usr/bin/env bash
# L25 — rebuild H6/H7 repro independently (KT-187 §6).
set -u
. "$(cd "$(dirname "$0")" && pwd)/lib.sh"

T=$BASE/h6h7

# ---------- H6 ----------
build_tree "$T/h6"
cat > "$T/h6/v2/kernel/vifast-core/src/evil_macro.rs" <<'EOF'
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
printf '\nmod evil_macro;\n' >> "$T/h6/v2/kernel/vifast-core/src/lib.rs"

# ---------- H7 ----------
build_tree "$T/h7"
cat > "$T/h7/v2/lib/vifast-abi/src/evil_inc_body.rs" <<'EOF'
pub fn evil_inc_body(m: &mut crate::Machine, t: crate::ObjId) -> Result<(), crate::KError> {
    m.wake(t)
}
EOF
cat >> "$T/h7/v2/lib/vifast-abi/src/lib.rs" <<'EOF'

#[macro_export]
macro_rules! evil_inc {
    () => {
        include!("../../../lib/vifast-abi/src/evil_inc_body.rs")
    };
}
EOF
cat >> "$T/h7/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'

pub fn evil_inc_probe(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    vifast_abi::evil_inc!();
    Ok(())
}
EOF

run "H6 current --strict" "$CUR" "$T/h6" --strict
run "H6 7e7e1b53 --strict" "$OLD" "$T/h6" --strict
run "H7 current --strict" "$CUR" "$T/h7" --strict
run "H7 7e7e1b53 --strict" "$OLD" "$T/h7" --strict
