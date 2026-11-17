#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-88 (D2-123.3) — dựng ảnh sản phẩm CUỐI hai lần ở HAI CARGO_TARGET_DIR khác nhau,
# trên cùng HEAD, cùng bộ đầu vào canonical. Chỉ ĐO, không sửa mã nguồn.
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
cd "$ROOT" || exit 1
LOGDIR="$ROOT/v2/agent/JOURNAL/kt88-logs"
EXTRA="--features product-shell,product-fs"

echo "### KT-88 build-log $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "### HEAD=$(git -C "$ROOT" rev-parse HEAD)"
echo "### dirty_scope=[v2/kernel v2/lib] lines=$(git -C "$ROOT" status --porcelain -- v2/kernel v2/lib | wc -l | tr -d ' ')"
echo "### VIFAST_REPRO_CARGO_EXTRA=[$EXTRA]"
echo "### features declared in Cargo.toml:"
sed -n '/^\[features\]/,/^\[\[bin\]\]/p' "$ROOT/v2/kernel/vifast-kbin-riscv64/Cargo.toml" | grep -E '^(product-shell|product-fs) =' 

for i in 1 2; do
    TD="/tmp/kt88-td$i"
    rm -rf "$TD"
    mkdir -p "$TD"
    echo
    echo "===== BUILD $i target_dir=$TD $(date -u +%Y-%m-%dT%H:%M:%SZ) ====="
    VIFAST_REPRO_CARGO_EXTRA="$EXTRA" \
    VIFAST_REPRO_LOG="$LOGDIR/kt88-cargo-build$i.log" \
        bash v2/tools/repro-build.sh --target-dir "$TD"
    rc=$?
    echo "----- BUILD $i exit=$rc"
    IMG="$TD/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64"
    if [ -f "$IMG" ]; then
        echo "----- BUILD $i sha256=$(shasum -a 256 "$IMG" | awk '{print $1}') size=$(wc -c <"$IMG" | tr -d ' ')"
        echo "----- BUILD $i digest: $(python3 v2/test/conformance/image_digest.py "$IMG")"
        echo "----- BUILD $i build-line-in-image:"
        LC_ALL=C grep -a -o -m2 '# build: rev=[0-9a-f]* dirty=[01]' "$IMG" | sort -u
    else
        echo "----- BUILD $i KHONG co anh $IMG"
    fi
done

echo
echo "===== so sanh byte hai ban build ====="
A=/tmp/kt88-td1/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
B=/tmp/kt88-td2/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
SA=$(shasum -a 256 "$A" 2>/dev/null | awk '{print $1}')
SB=$(shasum -a 256 "$B" 2>/dev/null | awk '{print $1}')
echo "build1_sha256=$SA"
echo "build2_sha256=$SB"
if [ -n "$SA" ] && [ "$SA" = "$SB" ]; then echo "CUNG_SHA=1"; else echo "CUNG_SHA=0"; fi
cmp "$A" "$B" && echo "cmp=identical"
echo "### ket thuc $(date -u +%Y-%m-%dT%H:%M:%SZ)"
