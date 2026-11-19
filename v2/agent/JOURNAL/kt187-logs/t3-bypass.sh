#!/usr/bin/env bash
# L25 — new bypass attempts (KT-187 §5).
set -u
. "$(cd "$(dirname "$0")" && pwd)/lib.sh"

T=$BASE/bypass
W='impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
'

# ---------- A: cfg_attr(not(test), path = ...) ----------
build_tree "$T/cfgattr"
cat >> "$T/cfgattr/v2/kernel/vifast-core/src/lib.rs" <<'EOF'
#[cfg(test)]
mod evil;
#[cfg_attr(not(test), path = "evil.rs")]
mod evil2;
EOF
printf '%s' "$W" > "$T/cfgattr/v2/kernel/vifast-core/src/evil.rs"
printf 'pub fn clean() {}\n' > "$T/cfgattr/v2/kernel/vifast-core/src/evil2.rs"

# ---------- A2: cfg_attr in a non-root product file ----------
build_tree "$T/cfgattr2"
cat >> "$T/cfgattr2/v2/kernel/vifast-core/src/lib.rs" <<'EOF'
#[cfg(test)]
mod evil;
EOF
printf '%s' "$W" > "$T/cfgattr2/v2/kernel/vifast-core/src/evil.rs"
printf 'pub fn clean() {}\n' > "$T/cfgattr2/v2/kernel/vifast-core/src/core_sched/evil2.rs" 2>/dev/null || true
mkdir -p "$T/cfgattr2/v2/kernel/vifast-core/src/core_sched"
printf 'pub fn clean() {}\n' > "$T/cfgattr2/v2/kernel/vifast-core/src/core_sched/evil2.rs"
cat >> "$T/cfgattr2/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'
#[cfg_attr(not(test), path = "../evil.rs")]
mod evil2;
EOF

# ---------- B: include! in macro_rules in v2/kernel ----------
build_tree "$T/macro-inc"
cat >> "$T/macro-inc/v2/kernel/vifast-core/src/lib.rs" <<'EOF'
#[cfg(test)]
mod evil;
EOF
printf '%s' "$W" > "$T/macro-inc/v2/kernel/vifast-core/src/evil.rs"
cat >> "$T/macro-inc/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'
macro_rules! evil_inc {
    () => { include!("evil.rs"); };
}
evil_inc!();
EOF

# ---------- C: mod declared in macro ----------
build_tree "$T/macro-mod"
cat >> "$T/macro-mod/v2/kernel/vifast-core/src/lib.rs" <<'EOF'
#[cfg(test)]
mod evil;
EOF
printf '%s' "$W" > "$T/macro-mod/v2/kernel/vifast-core/src/evil.rs"
cat >> "$T/macro-mod/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'
macro_rules! evil_mod {
    () => { mod evil; };
}
evil_mod!();
EOF

# ---------- D: paste! / concat_idents! ----------
build_tree "$T/paste"
cat >> "$T/paste/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'
fn duong_san_pham(m: &mut Machine, t: ObjId) {
    paste::paste! { let _ = m.[<wake>](t); }
}
EOF

build_tree "$T/concat-idents"
cat >> "$T/concat-idents/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'
fn duong_san_pham(m: &mut Machine, t: ObjId) {
    concat_idents!(m., wake, (t));
}
EOF

# ---------- E: #[path] absolute / backslash ----------
build_tree "$T/path-abs"
cat >> "$T/path-abs/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'
#[path = "/tmp/l25-184/evil_abs.rs"]
mod evil_abs;
EOF
printf '%s' "$W" > /tmp/l25-184/evil_abs.rs

build_tree "$T/path-backslash"
cat >> "$T/path-backslash/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'
#[path = "..\\..\\evil_win.rs"]
mod evil_win;
EOF

# ---------- F: include! in build.rs (legit OUT_DIR pattern) ----------
build_tree "$T/buildrs"
cat >> "$T/buildrs/v2/kernel/vifast-kbin-riscv64/build.rs" <<'EOF'
include!(concat!(env!("OUT_DIR"), "/generated.rs"));
EOF

# ---------- G: include! in #[cfg(test)] of product file, in-tree target ----------
build_tree "$T/cfgtest-include"
cat >> "$T/cfgtest-include/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'
#[cfg(test)]
mod tests {
    include!("test_helper.rs");
}
EOF
printf 'pub fn helper() -> u32 { 1 }\n' > "$T/cfgtest-include/v2/kernel/vifast-core/src/test_helper.rs"

# ---------- H: #[path] to nonexistent file ----------
build_tree "$T/path-missing"
cat >> "$T/path-missing/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'
#[path = "khong_ton_tai.rs"]
mod khong_ton_tai;
EOF

# ---------- I: #[path] with .. inside tree ----------
build_tree "$T/path-dotdot"
cat >> "$T/path-dotdot/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'
#[path = "../src/evil_ok.rs"]
mod evil_ok;
EOF
printf 'impl Machine {\n    fn duong_san_pham(&mut self, t: ObjId) {\n        let _ = self.wake_from(t);\n    }\n}\n' > "$T/path-dotdot/v2/kernel/vifast-core/src/evil_ok.rs"

# ---------- J: symlink inside tree -> inside tree ----------
build_tree "$T/symlink-in"
printf 'impl Machine {\n    fn duong_san_pham(&mut self, t: ObjId) {\n        let _ = self.wake_from(t);\n    }\n}\n' > "$T/symlink-in/v2/kernel/vifast-core/src/real_ok.rs"
ln -s real_ok.rs "$T/symlink-in/v2/kernel/vifast-core/src/link_ok.rs"
cat >> "$T/symlink-in/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'
#[path = "link_ok.rs"]
mod link_ok;
EOF

# ---------- K: symlink inside tree -> outside tree ----------
build_tree "$T/symlink-out"
printf '%s' "$W" > /tmp/l25-184/evil_link_target.rs
ln -s /tmp/l25-184/evil_link_target.rs "$T/symlink-out/v2/kernel/vifast-core/src/link_out.rs"
cat >> "$T/symlink-out/v2/kernel/vifast-core/src/core_sched.rs" <<'EOF'
#[path = "link_out.rs"]
mod link_out;
EOF

for d in cfgattr cfgattr2 macro-inc macro-mod paste concat-idents path-abs path-backslash buildrs cfgtest-include path-missing path-dotdot symlink-in symlink-out; do
  run "$d / current normal" "$CUR" "$T/$d"
  run "$d / current --strict" "$CUR" "$T/$d" --strict
done
