#!/bin/bash
# KT-195 — tái lập ba phát hiện F1/F2/F3 trên BẢN SAO CÂY THẬT.
# Dùng: ./repro-kt195.sh <base> <case>
#   base = bản sao cây ViFastOs (rsync --exclude target --exclude .git)
#   case = f1 | f2 | f3
# Yêu cầu: python3; cargo + toolchain 1.99.1 (chỉ để xác nhận RC=0).
set -u
BASE="${1:?base}"
CASE="${2:?case}"
D="/tmp/kt195-repro-$CASE"
PY=python3
CHK=v2/tools/check-wake-paths.py

rm -rf "$D"; rsync -a "$BASE/" "$D/"
CORE="$D/v2/kernel/vifast-core/src"

wake_file() {
  cat > "$1" <<'EOF'
// SPDX-License-Identifier: Apache-2.0 OR MIT
use crate::error::ObjId;
use crate::machine::Machine;
pub fn evil_wake(m: &mut Machine, t: ObjId) { let _ = m.wake(t); }
EOF
}

case "$CASE" in
  f1)
    # F1 — #[r#path] đặt trong lib.rs (crate root): chế độ thường XANH, --strict ĐỎ.
    wake_file "$CORE/evil_t.rs"
    printf '\n#[cfg(test)]\nmod evil_t;\n#[r#path = "evil_t.rs"]\nmod evil_x;\n' >> "$CORE/lib.rs"
    ;;
  f2)
    # F2 — `mod evil;` cùng dòng `}` đóng khối #[cfg(test)]: --strict XANH.
    wake_file "$CORE/evil.rs"
    printf '\n#[cfg(test)]\nfn kt195_helper() {\n    let _ = 1;\n} mod evil;\n' >> "$CORE/lib.rs"
    ;;
  f3)
    # F3 — cfg_attr lồng 40 tầng: E_LEX_DEPTH bị nuốt, cả hai chế độ XANH.
    wake_file "$CORE/evil_t.rs"
    printf '\n#[cfg(test)]\nmod evil_t;\n' >> "$CORE/lib.rs"
    inner='path = "evil_t.rs"'
    for _ in $(seq 1 40); do inner="cfg_attr(not(test), $inner)"; done
    printf '\n#[%s]\nmod evil_p;\n' "$inner" >> "$CORE/core_ipc.rs"
    ;;
  *) echo "case?"; exit 2;;
esac

cd "$D" || exit 1
echo "== $CASE =="
$PY "$CHK" 2>&1 | grep -E "E_LEX|E_STRICT|errors="; echo "thuong RC=${PIPESTATUS[0]}"
$PY "$CHK" --strict 2>&1 | grep -E "E_LEX|E_STRICT|errors="; echo "strict RC=${PIPESTATUS[0]}"
if command -v cargo >/dev/null 2>&1; then
  RUSTUP_TOOLCHAIN=1.99.1 cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-core --offline 2>&1 \
    | grep -E "evil|Finished|^error" | head -3
  echo "cargo RC=${PIPESTATUS[0]}"
fi
