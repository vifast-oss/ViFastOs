#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-258 — chèn **một biến thể tấn công** vào cây THẬT, đo `cargo check` + **ba** lượt kiểm
# (`check-wake-paths.py`, `--strict`, `check-wake-depinfo.py`), rồi **KHÔI PHỤC** bằng trap.
#
# Dùng: run-attack.sh <nhóm> <tên> <tệp-ca.sh>
#   <tệp-ca.sh> định nghĩa `chen()` (chèn). Tuỳ chọn:
#     `EXTRA_CARGO` — lệnh cargo thứ hai (chuỗi, chạy bằng `eval`) để chứng minh mã **được biên
#                     dịch** trong cấu hình khác (host workspace / `--features product-fs`);
#                     log `cargo-extra.log`, RC `cargo-extra-rc.txt`.
#     `go()`        — khôi phục thêm (hiếm khi cần; trap đã phủ).
#
# Chỉ tính là **đường lọt** khi `cargo` RC=0 **và** cả ba lượt kiểm `errors=0`.
# Exit: 0 = đã đo xong (kể cả khi cargo đỏ); 2 = dùng sai.
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-1.100.0}"
GROUP="${1:-?}"
NAME="${2:-?}"
SCRIPT="${3:-}"
BASE="$ROOT/v2/agent/JOURNAL/kt258-attack"
LOG="$BASE/logs/$NAME"
TSV="$BASE/kt258-attack.tsv"
[ -f "$SCRIPT" ] || {
    echo "run-attack: không thấy $SCRIPT" >&2
    exit 2
}
mkdir -p "$LOG"
cd "$ROOT" || exit 2
git status --porcelain >"$LOG/git-truoc.txt"

cleanup() {
    cd "$ROOT" 2>/dev/null || return 0
    # Chỉ khôi phục **cây sản phẩm** + **hai manifest workspace** (biến thể P1/P2 phải thêm
    # member/path-dep). Không `git checkout -- .` (sẽ xoá log/nhật ký chưa commit).
    git checkout -- v2/kernel v2/sim v2/lib v2/Cargo.toml v2/kernel/Cargo.toml 2>/dev/null || true
    # Crate ở **gốc mới** (`v2/drivers`, `v2/net`, `v2/foo`) + crate công cụ (`v2/tools`) là
    # tệp **chưa theo dõi** ⇒ `git clean` (không `-x`: giữ `target/` đã gitignore).
    git clean -fdq v2/kernel v2/sim v2/lib v2/drivers v2/net v2/foo v2/tools 2>/dev/null || true
    # `git clean -fd` **không** xoá tệp bị `.gitignore` (bài học KT-252 ca f3).
    rm -rf v2/kernel/*/src/target v2/sim/*/src/target v2/lib/*/src/target 2>/dev/null || true
    # Nguồn ngoài cây (P5 symlink / P7 path-dep ra ngoài).
    rm -rf /tmp/kt258-evil 2>/dev/null || true
    # Tệp nguồn phụ của ca P1e (include! ra ngoài crate).
    rm -f v2/agent/kt258-evil-src.rs 2>/dev/null || true
    git status --porcelain >"$LOG/git-sau.txt"
}
trap cleanup EXIT INT TERM

# shellcheck disable=SC1090
. "$SCRIPT"
if ! declare -F chen >/dev/null; then
    echo "run-attack: $SCRIPT thiếu hàm chen()" >&2
    exit 2
fi
chen

echo "=== $NAME — cargo check (riscv64gc-unknown-none-elf)"
cargo check --manifest-path v2/kernel/Cargo.toml --target riscv64gc-unknown-none-elf -q \
    >"$LOG/cargo.log" 2>&1
CARGORC=$?
echo "cargo RC=$CARGORC" | tee "$LOG/cargo-rc.txt"
[ "$CARGORC" -eq 0 ] || tail -5 "$LOG/cargo.log"

# Cấu hình **khác** (host / product-fs) — chứng minh mã thật sự được biên dịch ở đó.
if [ -n "${EXTRA_CARGO:-}" ]; then
    echo "=== $NAME — cargo phụ: $EXTRA_CARGO"
    eval "$EXTRA_CARGO" >"$LOG/cargo-extra.log" 2>&1
    echo "$?" >"$LOG/cargo-extra-rc.txt"
    echo "cargo phụ RC=$(cat "$LOG/cargo-extra-rc.txt")"
fi

L1="-"
L1S="-"
L2="-"
KET="cargo ĐỎ ⇒ không tính (biến thể không biên dịch được)"
if [ "$CARGORC" -eq 0 ]; then
    echo "=== $NAME — ba lượt kiểm (song song)"
    python3 v2/tools/check-wake-paths.py >"$LOG/lop1-thuong.log" 2>&1 &
    P1=$!
    python3 v2/tools/check-wake-paths.py --strict >"$LOG/lop1-strict.log" 2>&1 &
    P2=$!
    python3 v2/tools/check-wake-depinfo.py >"$LOG/lop2.log" 2>&1 &
    P3=$!
    wait "$P1"
    R1=$?
    wait "$P2"
    R2=$?
    wait "$P3"
    R3=$?
    L1="$(grep -o 'errors=[0-9]*' "$LOG/lop1-thuong.log" | tail -1 | cut -d= -f2)"
    L1S="$(grep -o 'errors=[0-9]*' "$LOG/lop1-strict.log" | tail -1 | cut -d= -f2)"
    L2="$(grep -o 'errors=[0-9]*' "$LOG/lop2.log" | tail -1 | cut -d= -f2)"
    echo "lop1 RC=$R1 errors=${L1:-?}  lop1s RC=$R2 errors=${L1S:-?}  lop2 RC=$R3 errors=${L2:-?}"
    if [ "${L1:-x}" = "0" ] && [ "${L1S:-x}" = "0" ] && [ "${L2:-x}" = "0" ]; then
        KET="**ĐƯỜNG LỌT** (cargo RC=0 + cả ba XANH)"
    else
        red=""
        [ "${L1:-x}" = "0" ] || red="$red l1(${L1:-?})"
        [ "${L1S:-x}" = "0" ] || red="$red l1s(${L1S:-?})"
        [ "${L2:-x}" = "0" ] || red="$red l2(${L2:-?})"
        KET="bị bắt:$red"
    fi
fi
printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$NAME" "$CARGORC" "${L1}" "${L1S}" "${L2}" "$KET" \
    >>"$TSV"
echo "# TSV: $TSV"
echo "$KET"
