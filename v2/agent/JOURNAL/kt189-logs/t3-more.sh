#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-189 / L28 — biến thể ngoặc, `mod` trong macro, dạng viết cách khác.
set -u
. "$(cd "$(dirname "$0")" && pwd)/lib.sh"
T=$BASE/b3
EVIL='use crate::{Machine, ObjId};

impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
'
mk_tree() {
    D=$1; MACROFILE=$2; CALL=$3
    build_full "$D"
    cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

mod m;
mod sub;
EOF
    printf '%s' "$MACROFILE" > "$D/v2/kernel/vifast-core/src/m.rs"
    printf 'pub fn clean() {}\n' > "$D/v2/kernel/vifast-core/src/evil.rs"
    cat > "$D/v2/kernel/vifast-core/src/sub.rs" <<'EOF'
#[cfg(test)]
mod evil;
mod s;
EOF
    mkdir -p "$D/v2/kernel/vifast-core/src/sub"
    printf '%s\n' "$CALL" > "$D/v2/kernel/vifast-core/src/sub/s.rs"
    printf '%s' "$EVIL" > "$D/v2/kernel/vifast-core/src/sub/evil.rs"
}
probe() {
    label=$1; D=$2
    {
      echo "# $label"
      run "$label CUR (chế độ thường)" "$CUR" "$D"
      run "$label CUR --strict" "$CUR" "$D" --strict
      echo "### $label cargo fmt --all --check"
      ( cd "$D/v2" && cargo fmt --all --check >/dev/null 2>&1; echo "fmt rc=$?" )
      echo "### $label cargo check -p vifast-core"
      ( cd "$D/v2/kernel" && cargo check -p vifast-core 2>&1 | grep -E 'warning: method|-->|Finished|^error' | head -5 )
    } >> "$BASE/logs/t3.log" 2>&1
}

: > "$BASE/logs/t3.log"

# (a) macro_rules! ngoặc VUÔNG
D=$T/bracket
mk_tree "$D" '#[macro_export]
macro_rules! evil_br [
    () => [
        #[path = "evil.rs"] mod x;
    ];
];
' 'crate::evil_br!();'
probe "T3a macro_rules! ngoặc VUÔNG + #[path]" "$D"

# (b) macro_rules! ngoặc VUÔNG + include!
D=$T/bracket-inc
mk_tree "$D" '#[macro_export]
macro_rules! evil_br_inc [
    () => [
        include!("evil.rs");
    ];
];
' 'crate::evil_br_inc!();'
probe "T3b macro_rules! ngoặc VUÔNG + include!" "$D"

# (c) `mod` khai trong macro + lời gọi #[cfg(not(test))] — tệp đích test-only theo đồ thị
D=$T/mod-macro
build_full "$D"
cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

mod m;
mod sub;
EOF
cat > "$D/v2/kernel/vifast-core/src/m.rs" <<'EOF'
#[macro_export]
macro_rules! evil_decl {
    () => { mod x; };
}
EOF
cat > "$D/v2/kernel/vifast-core/src/sub.rs" <<'EOF'
#[cfg(test)]
mod x;
#[cfg(not(test))]
crate::evil_decl!();
mod s;
EOF
mkdir -p "$D/v2/kernel/vifast-core/src/sub"
printf 'pub fn s() {}\n' > "$D/v2/kernel/vifast-core/src/sub/s.rs"
printf '%s' "$EVIL" > "$D/v2/kernel/vifast-core/src/sub/x.rs"
probe "T3c \`mod\` trong macro + lời gọi #[cfg(not(test))] (tệp test-only theo đồ thị)" "$D"

# (d) include! trong macro ngoặc tròn, KHÔNG có decoy cùng thư mục định nghĩa
D=$T/inc-paren-nodecoy
build_full "$D"
cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

mod m;
mod sub;
EOF
cat > "$D/v2/kernel/vifast-core/src/m.rs" <<'EOF'
#[macro_export]
macro_rules! evil_inc2 (
    () => (
        include!("evil.rs");
    );
);
EOF
cat > "$D/v2/kernel/vifast-core/src/sub.rs" <<'EOF'
#[cfg(test)]
mod evil;
mod s;
EOF
mkdir -p "$D/v2/kernel/vifast-core/src/sub"
printf 'crate::evil_inc2!();\n' > "$D/v2/kernel/vifast-core/src/sub/s.rs"
printf '%s' "$EVIL" > "$D/v2/kernel/vifast-core/src/sub/evil.rs"
probe "T3d include! trong macro ngoặc tròn, KHÔNG decoy (định nghĩa m.rs)" "$D"

echo "xong t3"
