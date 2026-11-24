#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0 OR MIT
# KT-129 — ĐO LẠI SÁU TIÊU CHÍ K6 TRÊN **ĐÚNG ẢNH** Agent 0 đã chốt (`a0287e4b…`).
#
# Vì sao có script này: Agent 0 chốt Q1 = (B) — "K6 ĐẠT, gắn với ảnh `a0287e4b`". Sáu tiêu chí của
# KT-88 đo trên ảnh `291502d2` (ảnh cũ hơn mười hai lần build). KT-126 đã chạy lại ma trận/parity/
# shell-edge/persist trên `a0287e4b` nhưng **không** để log riêng; KT-127 đã soak 1.000 trên chính
# ảnh đó. Script này đo lại **phần còn thiếu** trên đúng ảnh đã chốt và ghi log để văn bản đóng K6
# trích nguyên văn (`scope=`).
#
# KHÔNG build gì (đo ảnh có sẵn) ⇒ không chạm `v2/kernel/target/`, không đổi dấu vân tay.
# Dùng: bash v2/agent/JOURNAL/kt129-closure-measure.sh
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT" || exit 1
LOGDIR="$ROOT/v2/agent/JOURNAL/kt129-logs"
mkdir -p "$LOGDIR"
SHA="a0287e4ba926673ea0b5145b146e357e4f09898f2e8244b9ba8c51aa2cbf9f13"
FINAL="$ROOT/v2/artifacts/$SHA.bin"
[ -f "$FINAL" ] || { echo "kt129: không thấy ảnh $FINAL" >&2; exit 2; }
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"
. "$ROOT/v2/test/boot/build-rev.sh"

echo "### KT-129 CLOSURE-MEASURE $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "### image=$SHA"
echo "### image_bytes=$(wc -c <"$FINAL" | tr -d ' ')"
echo "### head_now=$(git rev-parse HEAD)"
echo "### qemu_con_sot_truoc=$(pgrep -f 'qemu-system-riscv64' | wc -l | tr -d ' ')"

# ---------------------------------------------------------------- F2: boot smp 1/2/4
echo
echo "===== F2 BOOT anh da chot smp 1/2/4 $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
. "$ROOT/v2/test/boot/qemu_guard.sh"
QG_DIR="${TMPDIR:-/tmp}"
QG_LOG="$LOGDIR/kt129-guard.log"
rm -f "$QG_LOG" 2>/dev/null
qg_init || { echo "qg_init FAIL"; exit 2; }
for SMP in 1 2 4; do
    L="$LOGDIR/kt129-boot-smp$SMP.log"
    PAY="$LOGDIR/.pay-boot-$SMP"
    printf 'help\nexit\n' >"$PAY"
    KT77_SEND_DELAY=0.05 qg_boot "$L" 300 "$ROOT/v2/test/parity/stdin_wrap.sh" "$PAY" \
        "$(command -v qemu-system-riscv64)" \
        -machine virt,aia=none,aclint=on -bios /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
        -kernel "$FINAL" -nographic -cpu rv64 -m 128M -smp "$SMP" -no-reboot
    echo "### boot smp=$SMP qemu_rc=$? secs=$QG_LAST_SECS log=$L"
    rm -f "$PAY"
    grep -a -m1 -E '^# build:' "$L"
    grep -a -m1 -E '^# product shell:' "$L" | cut -c1-90
    grep -a -m1 -E '^# shell: up' "$L"
    grep -a -m1 -E '^# smp: boot=' "$L" | cut -c1-70
    echo "### smp=$SMP shell_prompt_lines=$(grep -a -c '^shell>' "$L")"
    vbrev_gate "$L" "$ROOT/v2" "$FINAL" || echo "### REV-GATE REFUSE smp=$SMP"
done

# ---------------------------------------------------------------- F3: parity serial 1 & 4
echo
echo "===== F3 PARITY SERIAL tren anh da chot smp 1 va 4 $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
for SMP in 1 4; do
    bash v2/test/parity/run.sh --mode serial --image "$FINAL" --smp "$SMP" \
        --logdir "$LOGDIR/parity" --log-suffix -kt129
    echo "### parity smp=$SMP exit=$?"
    grep -a -E '^(ok|not ok) [0-9]+ - ' "$LOGDIR/parity/kt77a-serial-custom-smp$SMP-kt129.log" 2>/dev/null |
        grep -a -v 'k3 boot'
    grep -a -E '^# parity: (total|not_ok|KET_LUAN)' "$LOGDIR/parity/kt77a-serial-custom-smp$SMP-kt129.log" 2>/dev/null
done

# ---------------------------------------------------------------- F4: ngưỡng
echo
echo "===== F4 NGUONG tren anh da chot $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
echo "### image_bytes=$(wc -c <"$FINAL" | tr -d ' ') tran=262144"
bash v2/test/boot/budget.sh "$FINAL" product
echo "### budget_exit=$?"

# ---------------------------------------------------------------- F5: harness scan
echo
echo "===== F5 HARNESS SCAN tren anh da chot $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
NEGKT="$LOGDIR/ktest-proc-ca-am.bin"
if [ ! -f "$NEGKT" ]; then
    NEGSRC="$ROOT/v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-proc"
    [ -f "$NEGSRC" ] && cp "$NEGSRC" "$NEGKT"
fi
FEAT_PRODUCT="product-shell,product-fs" FEAT_HARNESS="ktest,user" \
    bash v2/agent/JOURNAL/kt77b-harness-scan.sh --image "$FINAL" --negative "$NEGKT"
echo "### harness_scan_exit=$?"

# ---------------------------------------------------------------- F6: ma trận
echo
echo "===== F6 MA TRAN ktest.sh 1 matrix (21 boot) $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
KT54_LOGDIR="$LOGDIR/ktest-matrix" bash v2/test/boot/ktest.sh 1 matrix
echo "### matrix_exit=$?"

# ---------------------------------------------------------------- F8: persist
echo
echo "===== F8 PERSIST 7 pha $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
KT75_LOGDIR="$LOGDIR/persist" bash v2/test/boot/persist.sh
echo "### persist_exit=$?"

echo
echo "### KT-129 XONG $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "### qemu_con_sot_sau=$(pgrep -f 'qemu-system-riscv64' | wc -l | tr -d ' ')"
