#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-88 (D2-123.3) — TIÊU CHÍ 1 + 2: boot ảnh sản phẩm CUỐI **từ tệp artifacts/<sha>.bin**
# ở smp 1/2/4 (tới `shell>`, có `# shell: up` / `harts=` / `# product shell:`), và chạy
# `v2/test/parity/run.sh --mode serial` trên **cùng tệp** ở smp 1 và 4 (6 mục `active`).
# Chỉ ĐO: không sửa mã nguồn, không sửa v2/test/**.
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
cd "$ROOT" || exit 1
LOGDIR="$ROOT/v2/agent/JOURNAL/kt88-logs"
SHA=38714b799fd27964785eba9aec2d8f591063d694f8a7bb9d9672f3abbd9decb2
IMG="$ROOT/v2/artifacts/$SHA.bin"
PLOGDIR="$LOGDIR/parity-serial"

. "$ROOT/v2/test/boot/build-rev.sh"

echo "### KT-88 T1/T2 $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "### HEAD=$(git -C "$ROOT" --no-optional-locks rev-parse HEAD)"
echo "### head12=$(git -C "$ROOT" --no-optional-locks rev-parse HEAD | cut -c1-12)"
echo "### dirty_scope_lines=$(git -C "$ROOT" --no-optional-locks status --porcelain --untracked-files=normal -- v2/kernel v2/lib | wc -l | tr -d ' ')"
echo "### image_path=$IMG"
echo "### image_sha256=$(shasum -a 256 "$IMG" | awk '{print $1}')"
echo "### image_bytes=$(wc -c <"$IMG" | tr -d ' ')"
echo "### image_build_line=$(LC_ALL=C grep -a -o -m1 '# build: rev=[0-9a-f]* dirty=[01]' "$IMG")"
echo "### qemu_con_sot_truoc=$(pgrep -f 'qemu-system-riscv64' | wc -l | tr -d ' ')"

rc_all=0
for SMP in 1 2 4; do
    echo
    echo "===== PARITY SERIAL smp=$SMP $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
    bash v2/test/parity/run.sh --mode serial --image "$IMG" --smp "$SMP" --logdir "$PLOGDIR"
    rc=$?
    echo "----- parity exit=$rc smp=$SMP"
    [ "$rc" -eq 0 ] || rc_all=1
    L="$PLOGDIR/kt77a-serial-custom-smp$SMP.log"
    echo "----- log=$L"
    if [ -f "$L" ]; then
        echo "----- markers (smp=$SMP):"
        grep -a -m3 -E '^# shell: up|^# product shell:|^# k3 shell: handoff' "$L" || echo "----- KHONG thay marker shell"
        echo "----- vbrev_gate tren log nay:"
        vbrev_gate "$L" "$ROOT/v2" "$IMG" || echo "----- REV-GATE REFUSE (log nay khong dung de do)"
        echo "----- TAP:"
        grep -a -E '^(ok|not ok)' "$L" || true
        echo "----- so dong log=$(wc -l <"$L" | tr -d ' ')"
    fi
done

echo
echo "### qemu_con_sot_sau=$(pgrep -f 'qemu-system-riscv64' | wc -l | tr -d ' ')"
echo "### ket thuc $(date -u +%Y-%m-%dT%H:%M:%SZ) rc_all=$rc_all"
exit "$rc_all"
