#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0 OR MIT
# KT-156 — **F3: CA ÂM "ảnh TẮT shell"** (OD-K6-2 tiêu chí 3): build ảnh **không** `product-shell`,
# chạy **kịch bản của F2** trên nó và đòi **ĐỎ**.
#
# **Vì sao KHÔNG dùng `--features product-fs`** như đề bài KT-156 ghi: trong cây này
#   `product-fs = ["product-shell", "vifast-plat-riscv64/block"]`
#   (`v2/kernel/vifast-kbin-riscv64/Cargo.toml:29`)
# ⇒ bật `product-fs` là **kéo theo** `product-shell`; build ra **đúng cùng sha** `e7149448…`
# (đo lần đầu trong `kt156-logs/kt156-master.log`, mục "F3 CA AM" — `noshell_sha=` trùng
# `image=`). Đó **không** phải ca âm.
#
# Ca âm đúng là build **không feature nào** — đúng comment của chính Cargo.toml:
#   "Ca âm của tiêu chí 3 = build **không** feature này (`bash v2/tools/repro-build.sh` mặc định)
#    ⇒ runner parity phải đỏ `exit 5`."
#
# Dùng: bash v2/agent/JOURNAL/kt156-f3-negative.sh
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT" || exit 1
LOGDIR="$ROOT/v2/agent/JOURNAL/kt156-logs"
mkdir -p "$LOGDIR"
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"
. "$ROOT/v2/test/boot/build-rev.sh"

echo "### KT-156 F3 CA AM (anh TAT shell) $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "### qemu_con_sot_truoc=$(pgrep -f 'qemu-system-riscv64' | wc -l | tr -d ' ')"

# 1) build ảnh **không feature** (mặc định của repro-build.sh)
echo "===== F3 BUILD anh TAT shell (khong feature) $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
NEGOUT="$(VIFAST_REPRO_LOG="$LOGDIR/kt156-build-noshell.log" bash v2/tools/repro-build.sh 2>&1)"
echo "### build_noshell_exit=$?"
printf '%s\n' "$NEGOUT" | grep -E '^REPRO ' | tail -1
NEGSHA="$(printf '%s\n' "$NEGOUT" | sed -n 's/.* sha256=\([0-9a-f]*\) .*/\1/p' | tail -1)"
NEGIMG="$(printf '%s\n' "$NEGOUT" | sed -n 's/.* image=\(.*\)$/\1/p' | tail -1)"
echo "### noshell_sha=$NEGSHA"
echo "### san_pham_sha=e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71"
[ "$NEGSHA" != "e714944864618dec661c75847c10bc908597810b80db9d37ae498229679c0a71" ] &&
    echo "### noshell_khac_anh_san_pham=1" || echo "### noshell_khac_anh_san_pham=0"
bash v2/tools/pin-artifact.sh "$NEGIMG" 2>&1 | tail -2
NEGPIN="$ROOT/v2/artifacts/$NEGSHA.bin"
echo "### noshell_pinned=$NEGPIN"

# 2) ảnh tắt shell KHÔNG được in `shell>` / `# product shell:`
echo "===== F3 BOOT anh TAT shell $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
. "$ROOT/v2/test/boot/qemu_guard.sh"
QG_DIR="${TMPDIR:-/tmp}"
QG_LOG="$LOGDIR/kt156-guard-f3.log"
rm -f "$QG_LOG" 2>/dev/null
qg_init || {
    echo "qg_init FAIL"
    exit 2
}
PAY="$LOGDIR/.f3-pay"
printf 'help\nexit\n' >"$PAY"
KT77_SEND_DELAY=0.05 qg_boot "$LOGDIR/kt156-f3-noshell-boot.log" 300 \
    "$ROOT/v2/test/parity/stdin_wrap.sh" "$PAY" \
    "$(command -v qemu-system-riscv64)" \
    -machine virt,aia=none,aclint=on -bios /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
    -kernel "$NEGPIN" -nographic -cpu rv64 -m 128M -smp 1 -no-reboot
echo "### f3_noshell_boot_exit=$?"
rm -f "$PAY"
echo "### f3_noshell_shell_prompt_lines=$(grep -a -c '^shell>' "$LOGDIR/kt156-f3-noshell-boot.log")"
grep -a -m1 -E '^# product shell:' "$LOGDIR/kt156-f3-noshell-boot.log" ||
    echo "### f3_khong_co_dong_product_shell=1"
grep -a -m1 -E '^# build:' "$LOGDIR/kt156-f3-noshell-boot.log"
grep -a -m1 -E '^# k3 halt' "$LOGDIR/kt156-f3-noshell-boot.log"

# 3) **kịch bản của F2** trên ảnh tắt shell phải **ĐỎ**
echo "===== F3 PARITY (kich ban F2) tren anh TAT shell $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
bash v2/test/parity/run.sh --mode serial --image "$NEGPIN" --smp 1 \
    --logdir "$LOGDIR/parity" --log-suffix -kt156-noshell
echo "### f3_parity_noshell_exit=$?"
grep -a -E '^# parity: (total|not_ok|KET_LUAN)' \
    "$LOGDIR/parity/kt77a-serial-custom-smp1-kt156-noshell.log" 2>/dev/null
echo "### f3_parity_noshell_not_ok=$(grep -a -c '^not ok ' "$LOGDIR/parity/kt77a-serial-custom-smp1-kt156-noshell.log" 2>/dev/null)"
echo "### KT-156 F3 XONG $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "### qemu_con_sot_sau=$(pgrep -f 'qemu-system-riscv64' | wc -l | tr -d ' ')"
