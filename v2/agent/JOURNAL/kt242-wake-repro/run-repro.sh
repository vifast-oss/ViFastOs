#!/usr/bin/env bash
# KT-242 — chèn repro vào cây THẬT, đo ba lượt kiểm, rồi KHÔI PHỤC (trap).
# Dùng: run-repro.sh <tên> <tệp-chèn.sh>
#   <tệp-chèn.sh> phải định nghĩa hàm `chen()` (chèn) và `go()` (khôi phục).
set -u
ROOT=/tmp/kt242-s2
export PATH="$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN=1.100.0
NAME="$1"; SCRIPT="$2"
LOG="$ROOT/v2/agent/JOURNAL/kt242-wake-repro/logs/$NAME"
mkdir -p "$LOG"

# Sao lưu trạng thái git để khôi phục chắc chắn.
cd "$ROOT"
git status --porcelain > "$LOG/git-truoc.txt"

cleanup() {
  cd "$ROOT"
  # Khôi phục mọi tệp đã theo dõi + xoá tệp mới của repro.
  git checkout -- . 2>/dev/null || true
  git clean -fdq v2/kernel v2/sim v2/lib 2>/dev/null || true
  echo "[khoi-phuc] git status:" > "$LOG/git-sau.txt"
  git status --porcelain >> "$LOG/git-sau.txt"
}
trap cleanup EXIT INT TERM

# shellcheck disable=SC1090
source "$SCRIPT"
chen

echo "=== $NAME — cargo check"
cargo check --manifest-path v2/kernel/Cargo.toml --target riscv64gc-unknown-none-elf -q > "$LOG/cargo.log" 2>&1
echo "cargo RC=$?" | tee "$LOG/cargo-rc.txt"
tail -5 "$LOG/cargo.log"

echo "=== $NAME — lớp 1 (thường)"
python3 v2/tools/check-wake-paths.py > "$LOG/lop1-thuong.log" 2>&1
echo "RC=$?" | tee -a "$LOG/lop1-thuong.log"
grep -E "^# check-wake-paths" "$LOG/lop1-thuong.log"

echo "=== $NAME — lớp 1 (--strict)"
python3 v2/tools/check-wake-paths.py --strict > "$LOG/lop1-strict.log" 2>&1
echo "RC=$?" | tee -a "$LOG/lop1-strict.log"
grep -E "^# check-wake-paths" "$LOG/lop1-strict.log"

echo "=== $NAME — lớp 2 (dep-info)"
python3 v2/tools/check-wake-depinfo.py > "$LOG/lop2.log" 2>&1
echo "RC=$?" | tee -a "$LOG/lop2.log"
grep -E "^# check-wake-depinfo|^not ok" "$LOG/lop2.log" | head -10
