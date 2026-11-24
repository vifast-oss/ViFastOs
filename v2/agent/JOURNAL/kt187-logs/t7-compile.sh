#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# KT-187 / L25 — chứng minh hai đường lọt **biên dịch được** (`cargo check -p vifast-core`
# RC=0) trong khi checker hiện tại **xanh** ở cả chế độ thường và `--strict`.
#
#   B1 — `#[cfg_attr(not(test), path = "…")]`: checker không thấy `#[path]` nào (regex đòi
#        `#[path` liền), nên tệp đích **ngoài** `v2/kernel`+`v2/sim` không bị quét.
#   B2 — `#[path]` trong thân `macro_rules!`: rustc giải theo **tệp gọi**, checker giải theo
#        **tệp định nghĩa** ⇒ đích thật (test-only, có `wake`) không bị quét.
#
# Chạy: bash v2/agent/JOURNAL/kt187-logs/t7-compile.sh
# Ghi:  compile-cfgattr.log, compile-macro-path.log
set -u
. "$(cd "$(dirname "$0")" && pwd)/lib.sh"
export PATH="$HOME/.cargo/bin:$PATH"

T=$BASE/compile

build_full() {
    rm -rf "$1"
    mkdir -p "$1/v2/tools"
    cp -R "$REPO/v2/kernel" "$1/v2/kernel"
    cp -R "$REPO/v2/sim" "$1/v2/sim"
    cp -R "$REPO/v2/lib" "$1/v2/lib"
    cp "$REPO/v2/Cargo.toml" "$1/v2/"
    cp "$REPO/v2/Cargo.lock" "$1/v2/"
    cp "$REPO/v2/rust-toolchain.toml" "$1/v2/"
    cp -R "$REPO/v2/tools/vifast-agentctl" "$1/v2/tools/"
}

# ---------------------------------------------------------------- B1: cfg_attr path
D=$T/cfgattr
build_full "$D"
cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

#[cfg_attr(not(test), path = "../../../lib/vifast-abi/src/evil_out.rs")]
mod evil2;
EOF
cat > "$D/v2/lib/vifast-abi/src/evil_out.rs" <<'EOF'
use crate::{Machine, ObjId};

impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
EOF
{
    echo "# B1 — #[cfg_attr(not(test), path = \"…\")] trỏ ra ngoài cây"
    echo "# cây: $D (bản sao v2/kernel + v2/sim + v2/lib + workspace v2)"
    echo
    run "B1 checker hiện tại (chế độ thường)" "$CUR" "$D"
    run "B1 checker hiện tại --strict" "$CUR" "$D" --strict
    echo "### B1 cargo fmt --all --check"
    ( cd "$D/v2" && cargo fmt --all --check 2>&1 | head -4 )
    echo "fmt rc=${PIPESTATUS[0]}"
    echo "### B1 cargo check -p vifast-core"
    ( cd "$D/v2/kernel" && cargo check -p vifast-core 2>&1 | grep -E 'warning: method|-->|Finished|^error' )
    echo "rc=${PIPESTATUS[0]}"
} > "$HERE/compile-cfgattr.log" 2>&1

# ---------------------------------------------------------------- B2: #[path] trong macro
D=$T/macro-path
build_full "$D"
cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

mod m;
mod sub;
EOF
cat > "$D/v2/kernel/vifast-core/src/m.rs" <<'EOF'
#[macro_export]
macro_rules! evil_mod {
    () => {
        #[path = "evil.rs"]
        mod x;
    };
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
cat > "$D/v2/kernel/vifast-core/src/sub/evil.rs" <<'EOF'
use crate::{Machine, ObjId};

impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
EOF
{
    echo "# B2 — #[path] trong thân macro_rules! (rustc giải theo tệp gọi)"
    echo "# cây: $D (bản sao v2/kernel + v2/sim + v2/lib + workspace v2)"
    echo
    run "B2 checker hiện tại (chế độ thường)" "$CUR" "$D"
    run "B2 checker hiện tại --strict" "$CUR" "$D" --strict
    echo "### B2 cargo fmt --all --check"
    ( cd "$D/v2" && cargo fmt --all --check 2>&1 | head -4 )
    echo "fmt rc=${PIPESTATUS[0]}"
    echo "### B2 cargo check -p vifast-core"
    ( cd "$D/v2/kernel" && cargo check -p vifast-core 2>&1 | grep -E 'warning: method|-->|Finished|^error' )
    echo "rc=${PIPESTATUS[0]}"
} > "$HERE/compile-macro-path.log" 2>&1


# ---------------------------------------------------------------- B3: include! trong macro
D=$T/macro-include
build_full "$D"
cat >> "$D/v2/kernel/vifast-core/src/lib.rs" <<'EOF'

mod m;
mod sub;
EOF
cat > "$D/v2/kernel/vifast-core/src/m.rs" <<'EOF'
#[macro_export]
macro_rules! evil_inc {
    () => {
        include!("evil.rs");
    };
}
EOF
printf 'pub fn clean() {}\n' > "$D/v2/kernel/vifast-core/src/evil.rs"
cat > "$D/v2/kernel/vifast-core/src/sub.rs" <<'EOF'
#[cfg(test)]
mod evil;
mod s;
EOF
mkdir -p "$D/v2/kernel/vifast-core/src/sub"
printf 'crate::evil_inc!();\n' > "$D/v2/kernel/vifast-core/src/sub/s.rs"
cat > "$D/v2/kernel/vifast-core/src/sub/evil.rs" <<'EOF'
use crate::{Machine, ObjId};

impl Machine {
    fn duong_san_pham(&mut self, t: ObjId) {
        let _ = self.wake(t);
    }
}
EOF
{
    echo "# B3 — include! trong thân macro_rules! (rustc giải theo tệp gọi)"
    echo "# cây: $D (bản sao v2/kernel + v2/sim + v2/lib + workspace v2)"
    echo
    run "B3 checker hiện tại (chế độ thường)" "$CUR" "$D"
    run "B3 checker hiện tại --strict" "$CUR" "$D" --strict
    echo "### B3 cargo fmt --all --check"
    ( cd "$D/v2" && cargo fmt --all --check 2>&1 | head -4 )
    echo "fmt rc=${PIPESTATUS[0]}"
    echo "### B3 cargo check -p vifast-core"
    ( cd "$D/v2/kernel" && cargo check -p vifast-core 2>&1 | grep -E 'warning: method|-->|Finished|^error' )
    echo "rc=${PIPESTATUS[0]}"
} > "$HERE/compile-macro-include.log" 2>&1

echo "xong: $HERE/compile-cfgattr.log, $HERE/compile-macro-path.log, $HERE/compile-macro-include.log"
