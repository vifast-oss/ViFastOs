#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-88 (D2-123.3) — CHẠY TOÀN BỘ PHÉP ĐO TRÊN HEAD ĐÓNG BĂNG (lead chốt phương án (1)).
# Dùng: bash kt88-20-freeze-run.sh <HEAD-40-hex-do-lead-chot>
# Từ chối chạy (exit 9) nếu HEAD hiện tại khác HEAD đóng băng hoặc cây nguồn bẩn — fail-closed,
# không tắt/không lách cổng KT-86.
# Thứ tự (không song song soak/ma trận/build):
#   F0 kiểm HEAD + dirty
#   F1 build 2 lần ở 2 CARGO_TARGET_DIR -> CÙNG sha (canonical `product-shell,product-fs`)
#   F2 boot ảnh cuối ở smp 1/2/4 (marker shell) — 3 boot "ảnh sản phẩm 3 cấu hình"
#   F3 parity --mode serial ở smp 1 và 4 (6 mục active)
#   F4 ngưỡng: size ảnh, .data+.bss, (size_of::<Machine> lấy từ ma trận/boot ktest)
#   F5 harness scan (product 0 hit, ca âm > 0, probe gate ktest=0 umode=1)
#   F6 MA TRẬN ktest.sh 1 matrix (7 ảnh × 1/2/4 = 21 boot)
#   F7 SOAK 1.000 (3 lane)
#   F8 PERSIST 7 pha
#   F9 đọc lại sau reboot (2 boot, cùng tệp đĩa) trên ảnh cuối
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
cd "$ROOT" || exit 1
LOGDIR="$ROOT/v2/agent/JOURNAL/kt88-logs"
FROZEN="${1:-}"
[ -n "$FROZEN" ] || { echo "can HEAD dong bang (40 hex) lam tham so 1"; exit 2; }
NOW="$(git rev-parse HEAD)"
. "$ROOT/v2/test/boot/build-rev.sh"
DIRTY="$(vbrev_dirty "$ROOT")"

echo "### KT-88 FREEZE-RUN $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "### frozen_head=$FROZEN"
echo "### head_now=$NOW"
echo "### head12=$(printf '%s' "$NOW" | cut -c1-12)"
echo "### dirty_scope=[$VBREV_SCOPE] dirty=$DIRTY"
if [ "$NOW" != "$FROZEN" ]; then
    echo "### TU CHOI: HEAD hien tai KHAC HEAD dong bang — khong do (fail-closed)"
    exit 9
fi
if [ "$DIRTY" != "0" ]; then
    echo "### TU CHOI: cay nguon BAN trong [$VBREV_SCOPE] — khong do (fail-closed)"
    vbrev_dirty_files "$ROOT" | head -5
    exit 9
fi
echo "### qemu_con_sot_truoc=$(pgrep -f 'qemu-system-riscv64' | wc -l | tr -d ' ')"
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

# ---------------------------------------------------------------- F1: build 2 lần
echo
echo "===== F1 BUILD 2 LAN (canonical product-shell,product-fs) $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
for i in 1 2; do
    TD="/tmp/kt88-frz-$i"
    HB="$(git rev-parse HEAD)"
    rm -rf "$TD"; mkdir -p "$TD"
    VIFAST_REPRO_CARGO_EXTRA="--features product-shell,product-fs" \
    VIFAST_REPRO_LOG="$LOGDIR/kt88-frz-cargo-$i.log" \
        bash v2/tools/repro-build.sh --target-dir "$TD"
    echo "----- build$i exit=$? head_truoc=$HB head_sau=$(git rev-parse HEAD)"
    if [ "$HB" != "$(git rev-parse HEAD)" ]; then
        echo "### TU CHOI: HEAD DOI TRONG LUC BUILD $i ($HB -> $(git rev-parse HEAD)) — sha khong con nghia"
        exit 9
    fi
    shasum -a 256 "$TD/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64"
done
A=/tmp/kt88-frz-1/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
B=/tmp/kt88-frz-2/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
SA="$(shasum -a 256 "$A" | awk '{print $1}')"
SB="$(shasum -a 256 "$B" | awk '{print $1}')"
echo "### build1_sha256=$SA"
echo "### build2_sha256=$SB"
if [ "$SA" != "$SB" ]; then echo "### CUNG_SHA=0 -> DUNG (khong tai lap duoc)"; exit 1; fi
echo "### CUNG_SHA=1 cmp=$(cmp "$A" "$B" >/dev/null 2>&1 && echo identical || echo DIFFER)"
FINAL="$ROOT/v2/artifacts/$SA.bin"
cp "$A" "$FINAL"; chmod 555 "$FINAL"
cp "$A" "$ROOT/artifacts/$SA.bin"; chmod 555 "$ROOT/artifacts/$SA.bin"
echo "### FINAL_IMAGE=$FINAL"
echo "### FINAL_SHA256=$(shasum -a 256 "$FINAL" | awk '{print $1}') size=$(wc -c <"$FINAL" | tr -d ' ')"
echo "### FINAL_DIGEST=$(python3 v2/test/conformance/image_digest.py "$FINAL")"
echo "### FINAL_BUILD_LINE=$(LC_ALL=C grep -a -o -m1 '# build: rev=[0-9a-f]* dirty=[01]' "$FINAL")"
echo "### FINAL_PIN=$(LC_ALL=C grep -a -c "rev=$(printf '%s' "$NOW" | cut -c1-12) dirty=0" "$FINAL")"

# ---------------------------------------------------------------- F2: boot 3 cấu hình
echo
echo "===== F2 BOOT anh cuoi smp 1/2/4 (marker shell) $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
. "$ROOT/v2/test/boot/qemu_guard.sh"
QG_DIR="${TMPDIR:-/tmp}"
QG_LOG="$LOGDIR/frz-guard.log"
rm -f "$QG_LOG" 2>/dev/null
qg_init || { echo "qg_init FAIL"; exit 2; }
for SMP in 1 2 4; do
    L="$LOGDIR/boot-final-smp$SMP.log"
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
echo "===== F3 PARITY SERIAL tren anh cuoi smp 1 va 4 $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
for SMP in 1 4; do
    bash v2/test/parity/run.sh --mode serial --image "$FINAL" --smp "$SMP" \
        --logdir "$LOGDIR/parity-final" --log-suffix -final
    echo "### parity_final smp=$SMP exit=$?"
    grep -a -E '^(ok|not ok) [0-9]+ - ' "$LOGDIR/parity-final/kt77a-serial-custom-smp$SMP-final.log" 2>/dev/null |
        grep -a -v 'k3 boot'
    grep -a -E '^# parity: (total|not_ok|KET_LUAN)' "$LOGDIR/parity-final/kt77a-serial-custom-smp$SMP-final.log" 2>/dev/null
done

# ---------------------------------------------------------------- F4: ngưỡng
echo
echo "===== F4 NGUONG tren anh cuoi $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
echo "### image_bytes=$(wc -c <"$FINAL" | tr -d ' ') tran=262144"
bash v2/test/boot/budget.sh "$FINAL" product
echo "### budget_exit=$?"

# ---------------------------------------------------------------- F5: harness scan
echo
echo "===== F5 HARNESS SCAN tren anh cuoi $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
NEGKT="$LOGDIR/ktest-proc-ca-am.bin"
if [ ! -f "$NEGKT" ]; then
    NEGSRC="$ROOT/v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-proc"
    [ -f "$NEGSRC" ] && cp "$NEGSRC" "$NEGKT"
fi
if [ -f "/tmp/kt88-hs/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-proc" ]; then
    cp /tmp/kt88-hs/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-proc "$NEGKT"
fi
FEAT_PRODUCT="product-shell,product-fs" FEAT_HARNESS="ktest,user" \
    bash v2/agent/JOURNAL/kt77b-harness-scan.sh --image "$FINAL" --negative "$NEGKT"
echo "### harness_scan_exit=$?"

# ---------------------------------------------------------------- F6: ma trận
echo
echo "===== F6 MA TRAN ktest.sh 1 matrix (21 boot) $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
KT54_LOGDIR="$LOGDIR/ktest-matrix" bash v2/test/boot/ktest.sh 1 matrix
echo "### matrix_exit=$?"
tail -25 "$LOGDIR/ktest-matrix/kt54-summary.log" 2>/dev/null || true

# ---------------------------------------------------------------- F7: soak
echo
echo "===== F7 SOAK 1000 (3 lane) $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
SOAK_LOGDIR="$LOGDIR/soak" bash v2/test/boot/soak.sh 1000 3
echo "### soak_exit=$?"

# ---------------------------------------------------------------- F8: persist
echo
echo "===== F8 PERSIST 7 pha $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
KT75_LOGDIR="$LOGDIR/persist" bash v2/test/boot/persist.sh
echo "### persist_exit=$?"

# ---------------------------------------------------------------- F9: đọc lại sau reboot
echo
echo "===== F9 DOC LAI SAU REBOOT (2 boot, anh cuoi) $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
RDIR="$LOGDIR/persist-2boot-final"; mkdir -p "$RDIR"
DISK="$RDIR/kt88-final.img"
rm -f "$DISK"; dd if=/dev/zero of="$DISK" bs=4096 count=1024 >/dev/null 2>&1
send_boot() {
    local log="$1" payload="$2" pay="$RDIR/.pay.$$"
    printf '%b' "$payload" >"$pay"
    KT77_SEND_DELAY=0.05 qg_boot "$log" 300 "$ROOT/v2/test/parity/stdin_wrap.sh" "$pay" \
        "$(command -v qemu-system-riscv64)" \
        -machine virt,aia=none,aclint=on -bios /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
        -kernel "$FINAL" -nographic -cpu rv64 -m 128M -smp 1 -no-reboot \
        -drive "file=$DISK,format=raw,if=none,id=hd0" -device virtio-blk-pci,drive=hd0
    local rc=$?
    rm -f "$pay"
    return "$rc"
}
send_boot "$RDIR/boot1.log" 'write /p.txt hello77\nexit\n'
echo "### boot1_rc=$? disk_sha=$(shasum -a 256 "$DISK" | awk '{print $1}')"
grep -a -E '^# (product fs:|shell: write)' "$RDIR/boot1.log" | head -3
D1="$(shasum -a 256 "$DISK" | awk '{print $1}')"
send_boot "$RDIR/boot2.log" 'cat /p.txt\nexit\n'
echo "### boot2_rc=$? disk_sha=$(shasum -a 256 "$DISK" | awk '{print $1}')"
grep -a -E '^# (product fs:|shell: cat)' "$RDIR/boot2.log" | head -3
D2="$(shasum -a 256 "$DISK" | awk '{print $1}')"
echo "### disk_sha_khong_doi=$([ "$D1" = "$D2" ] && echo 1 || echo 0)"
grep -a -q '^# shell: cat ok bytes=7 first=hello77$' "$RDIR/boot2.log" && echo "### boot2_doc_lai=1" || echo "### boot2_doc_lai=0"
vbrev_gate "$RDIR/boot2.log" "$ROOT/v2" "$FINAL" || echo "### REV-GATE REFUSE boot2"

echo
echo "### qemu_con_sot_sau=$(pgrep -f 'qemu-system-riscv64' | wc -l | tr -d ' ')"
echo "### KET THUC FREEZE-RUN $(date -u +%Y-%m-%dT%H:%M:%SZ) final_sha=$SA"
