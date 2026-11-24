#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-88 (D2-123.3) — CHUỖI ĐO TUẦN TỰ (không song song build/ma trận/soak — D2-24/D2-72):
#   P1 ca âm (2 build canonical + 2 parity)      -> kt88-06-negatives.sh
#   P2 MA TRẬN ktest.sh 7 ảnh × smp 1/2/4 = 21   (build ảnh test nằm trong runner)
#   P3 harness scan trên ảnh cuối + ca âm ktest-proc
#   P4 SOAK 1.000 vòng (3 lane)
#   P5 PERSIST 7 pha / 10 boot
# Mỗi pha ghi log riêng dưới v2/agent/JOURNAL/kt88-logs/ và in mốc thời gian.
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
cd "$ROOT" || exit 1
LOGDIR="$ROOT/v2/agent/JOURNAL/kt88-logs"
FINAL_SHA=38714b799fd27964785eba9aec2d8f591063d694f8a7bb9d9672f3abbd9decb2
PROD_IMG="v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-proc"

hdr() {
    echo
    echo "############ $1 $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "### HEAD=$(git rev-parse HEAD) head12=$(git rev-parse HEAD | cut -c1-12)"
    echo "### dirty_scope_lines=$(git status --porcelain --untracked-files=normal -- v2/kernel v2/lib | wc -l | tr -d ' ')"
    echo "### qemu_con_sot=$(pgrep -f 'qemu-system-riscv64' | wc -l | tr -d ' ')"
}

echo "### KT-88 chain $(date -u +%Y-%m-%dT%H:%M:%SZ) final_sha256=$FINAL_SHA"

hdr "P1 CA AM"
bash v2/agent/JOURNAL/kt88-logs/kt88-06-negatives.sh >"$LOGDIR/kt88-10-negatives.log" 2>&1
echo "### P1 exit=$?"

hdr "P2 MA TRAN ktest.sh 1 matrix (21 boot)"
KT54_LOGDIR="$LOGDIR/ktest-matrix" bash v2/test/boot/ktest.sh 1 matrix >"$LOGDIR/kt88-11-matrix.log" 2>&1
echo "### P2 exit=$? (0 = 21/21 sach)"
tail -25 "$LOGDIR/kt88-11-matrix.log"

hdr "P3 HARNESS SCAN anh cuoi + ca am ktest-proc"
FEAT_PRODUCT="product-shell,product-fs" FEAT_HARNESS="ktest,user" \
    bash v2/agent/JOURNAL/kt77b-harness-scan.sh \
    --image "v2/artifacts/$FINAL_SHA.bin" --negative "$PROD_IMG" >"$LOGDIR/kt88-12-harness-scan.log" 2>&1
echo "### P3 exit=$?"
cat "$LOGDIR/kt88-12-harness-scan.log"

hdr "P4 SOAK 1000 (3 lane)"
SOAK_LOGDIR="$LOGDIR/soak" bash v2/test/boot/soak.sh 1000 3 >"$LOGDIR/kt88-13-soak.log" 2>&1
echo "### P4 exit=$? (0 = 0 loi/1000)"
tail -30 "$LOGDIR/kt88-13-soak.log"

hdr "P5 PERSIST 7 pha"
KT75_LOGDIR="$LOGDIR/persist" bash v2/test/boot/persist.sh >"$LOGDIR/kt88-14-persist.log" 2>&1
echo "### P5 exit=$?"
tail -30 "$LOGDIR/kt88-14-persist.log"

hdr "KET THUC CHUOI"
echo "### qemu_con_sot_cuoi=$(pgrep -f 'qemu-system-riscv64' | wc -l | tr -d ' ')"
echo "### done $(date -u +%Y-%m-%dT%H:%M:%SZ)"
