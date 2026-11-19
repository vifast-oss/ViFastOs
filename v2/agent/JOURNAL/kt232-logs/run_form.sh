#!/bin/bash
# KT-232 — chạy một dạng: cargo check + lớp 1 + lớp 1 --strict + lớp 2, rồi hoàn nguyên.
# usage: run_form.sh <tên> <tệp nguồn>
set -u
cd /private/tmp/night3-L55-kt232
export VIFAST_ROLE=lead
export PATH="$HOME/.cargo/bin:$PATH"
NAME="$1"; SRC="$2"
LOGDIR=v2/agent/JOURNAL/kt232-logs
TARGET=v2/kernel/vifast-core/src/kt232_x.rs
LIB=v2/kernel/vifast-core/src/lib.rs

cp "$SRC" "$TARGET"
python3 - "$LIB" <<'PY'
import sys
p = sys.argv[1]
s = open(p).read()
assert "mod kt232_x;" not in s, "mod đã có"
s = s.replace("mod tlb;\n", "mod tlb;\nmod kt232_x;\n", 1)
open(p, "w").write(s)
PY

cargo check --manifest-path v2/kernel/Cargo.toml --workspace --target riscv64gc-unknown-none-elf \
    > "$LOGDIR/$NAME-cargo.log" 2>&1
CARGO_RC=$?

python3 v2/tools/check-wake-paths.py > "$LOGDIR/$NAME-l1.log" 2>&1 &
P1=$!
python3 v2/tools/check-wake-paths.py --strict > "$LOGDIR/$NAME-l1s.log" 2>&1 &
P2=$!
python3 v2/tools/check-wake-depinfo.py --root . > "$LOGDIR/$NAME-l2.log" 2>&1 &
P3=$!
wait $P1; L1=$?
wait $P2; L1S=$?
wait $P3; L2=$?

git checkout -- "$LIB"
rm -f "$TARGET"

echo "$NAME cargo=$CARGO_RC l1=$L1 l1s=$L1S l2=$L2"
