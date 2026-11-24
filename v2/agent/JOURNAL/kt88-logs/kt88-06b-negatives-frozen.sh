#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-88 (D2-123.3) — TIÊU CHÍ 3 (CA ÂM), trên HEAD đóng băng d40f1e63a61d:
#   (a) build **không** feature shell  ⇒ parity phải ĐỎ `khong_co_shell` + mã thoát riêng (5).
#       Ảnh này cũng là **baseline không-shell MỚI** (sau KT-86 nhúng `# build:`; baseline cũ
#       c1072d72 chỉ còn giá trị lịch sử).
#   (b) build **có** shell nhưng **không** FS (`--features product-shell`) ⇒ `ls`/`cat` ĐỎ,
#       4 lệnh còn lại (help/echo/ps/kill) XANH.
# Chỉ ĐO, không sửa mã nguồn; mỗi ảnh dựng bằng pipeline canonical (repro-build.sh) ở
# CARGO_TARGET_DIR **ngoài** cây nguồn.
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
cd "$ROOT" || exit 1
LOGDIR="$ROOT/v2/agent/JOURNAL/kt88-logs"
PLOG="$LOGDIR/parity-neg-frz"

head12="$(git rev-parse HEAD | cut -c1-12)"
echo "### KT-88 T3 ca am $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "### HEAD=$(git rev-parse HEAD) head12=$head12"
echo "### dirty_scope_lines=$(git status --porcelain --untracked-files=normal -- v2/kernel v2/lib | wc -l | tr -d ' ')"
echo "### qemu_con_sot_truoc=$(pgrep -f 'qemu-system-riscv64' | wc -l | tr -d ' ')"

build_case() { # $1=nhãn $2=extra-cargo-args $3=target-dir
    local label="$1" extra="$2" td="$3" out=""
    rm -rf "$td"
    mkdir -p "$td"
    echo
    echo "===== BUILD ca am [$label] extra=[$extra] td=$td $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
    VIFAST_REPRO_CARGO_EXTRA="$extra" VIFAST_REPRO_LOG="$LOGDIR/kt88-cargo-$label.log" \
        bash v2/tools/repro-build.sh --target-dir "$td"
    echo "----- exit=$? label=$label"
}

# ---------------------------------------------------------------- (a) KHÔNG shell
build_case noshell "" /tmp/kt88-frz-noshell
NOSHELL_IMG=/tmp/kt88-frz-noshell/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
NOSHELL_SHA="$(shasum -a 256 "$NOSHELL_IMG" | awk '{print $1}')"
NOSHELL_SIZE="$(wc -c <"$NOSHELL_IMG" | tr -d ' ')"
NOSHELL_DIGEST="$(python3 v2/test/conformance/image_digest.py "$NOSHELL_IMG")"
cp "$NOSHELL_IMG" "v2/artifacts/$NOSHELL_SHA.bin"
chmod 555 "v2/artifacts/$NOSHELL_SHA.bin"
echo "### BASELINE_MOI_KHONG_SHELL sha256=$NOSHELL_SHA size=$NOSHELL_SIZE $NOSHELL_DIGEST"
echo "### BASELINE_MOI_KHONG_SHELL build_line=$(LC_ALL=C grep -a -o -m1 '# build: rev=[0-9a-f]* dirty=[01]' "$NOSHELL_IMG")"
echo "### BASELINE_MOI_KHONG_SHELL artifact=v2/artifacts/$NOSHELL_SHA.bin sha_artifact=$(shasum -a 256 "v2/artifacts/$NOSHELL_SHA.bin" | awk '{print $1}')"
echo "### BASELINE_CU c1072d72 = 80.6K, TRUOC KT-86 (khong co dong '# build:') -> chi con gia tri lich su"

echo
echo "===== PARITY tren anh KHONG shell (ky vong exit=5 khong_co_shell) $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
bash v2/test/parity/run.sh --mode serial --image "v2/artifacts/$NOSHELL_SHA.bin" --smp 1 --logdir "$PLOG"
NOSHELL_RC=$?
echo "### NOSHELL_PARITY_EXIT=$NOSHELL_RC (5 = khong_co_shell theo run.sh)"
NSLOG="$PLOG/kt77a-serial-custom-smp1.log"
[ -f "$NSLOG" ] && grep -a -E 'khong_co_shell|khong thay shell|^# run.sh: |not ok' "$NSLOG" | head -10

# ---------------------------------------------------------------- (b) CÓ shell, KHÔNG FS
build_case shellnofs "--features product-shell" /tmp/kt88-frz-shellnofs
SHELLNOFS_IMG=/tmp/kt88-frz-shellnofs/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
SNF_SHA="$(shasum -a 256 "$SHELLNOFS_IMG" | awk '{print $1}')"
SNF_SIZE="$(wc -c <"$SHELLNOFS_IMG" | tr -d ' ')"
SNF_DIGEST="$(python3 v2/test/conformance/image_digest.py "$SHELLNOFS_IMG")"
cp "$SHELLNOFS_IMG" "v2/artifacts/$SNF_SHA.bin"
chmod 555 "v2/artifacts/$SNF_SHA.bin"
echo "### ANH_SHELL_KHONG_FS sha256=$SNF_SHA size=$SNF_SIZE $SNF_DIGEST artifact=v2/artifacts/$SNF_SHA.bin"

echo
echo "===== PARITY tren anh CO shell KHONG FS (ky vong ls/cat DO, 4 lenh kia XANH) $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
bash v2/test/parity/run.sh --mode serial --image "v2/artifacts/$SNF_SHA.bin" --smp 1 --logdir "$PLOG" --log-suffix -nofs
SNF_RC=$?
echo "### SHELLNOFS_PARITY_EXIT=$SNF_RC"
SNFLOG="$PLOG/kt77a-serial-custom-smp1-nofs.log"
[ -f "$SNFLOG" ] && grep -a -E '^(ok|not ok) [0-9]' "$SNFLOG"

echo
echo "### qemu_con_sot_sau=$(pgrep -f 'qemu-system-riscv64' | wc -l | tr -d ' ')"
echo "### ket thuc $(date -u +%Y-%m-%dT%H:%M:%SZ)"
