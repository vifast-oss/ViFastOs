#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-07 — boot test riscv64 (K0.5): build ảnh kbin, boot QEMU N lần, đòi TAP 1 test,
# bắt buộc 0 lỗi, và đo `.data + .bss` (cổng cứng ≤ 256 KiB [TK], QD §3.5).
# Dùng: bash v2/test/boot/run.sh [N]      (mặc định N=50, N nhỏ khi thử)
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"        # v2/
REPO_ROOT="$(cd "$V2_ROOT/.." && pwd)"
KBIN="$V2_ROOT/kernel/vifast-kbin-riscv64"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
TARGET="riscv64gc-unknown-none-elf"
IMAGE="$V2_ROOT/kernel/target/$TARGET/release/vifast-kbin-riscv64"
BUDGET_BYTES=$((256 * 1024))
N="${1:-50}"

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"
command -v cargo >/dev/null 2>&1 || { echo "boot-test: không có cargo" >&2; exit 127; }

QEMU="$(command -v qemu-system-riscv64 || true)"
[ -n "$QEMU" ] || { echo "boot-test: không có qemu-system-riscv64" >&2; exit 127; }

# OpenSBI: nạp trực tiếp (QD §1 Q-F(1) — tới K6 chỉ riscv64).
SBI="${OPENSBI:-}"
if [ -z "$SBI" ]; then
    for c in /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
        /usr/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
        /usr/local/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin; do
        [ -f "$c" ] && { SBI="$c"; break; }
    done
fi
[ -f "${SBI:-/nonexistent}" ] || { echo "boot-test: không tìm thấy OpenSBI (đặt OPENSBI=...)" >&2; exit 127; }

echo "# boot-test: N=$N  qemu=$(basename "$QEMU")  opensbi=$(basename "$SBI")"

if [ ! -f "$MANIFEST" ]; then
    echo "boot-test: chưa có $MANIFEST" >&2
    exit 1
fi

echo "# build $TARGET"
if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-riscv64 --target "$TARGET" --release >/tmp/v2-kbin-build.log 2>&1; then
    echo "boot-test: build FAIL (xem /tmp/v2-kbin-build.log)" >&2
    tail -20 /tmp/v2-kbin-build.log >&2
    exit 1
fi
[ -f "$IMAGE" ] || { echo "boot-test: không thấy ảnh $IMAGE" >&2; exit 1; }

# Cổng cứng: .data + .bss ≤ 256 KiB [TK] — đo ngay ở K0.5 (QD §3.5, QD §9 Q8, thiết kế §8).
# Báo RIÊNG cỡ image và .stack để so sánh được (ngưỡng 256 KiB chỉ áp cho .data+.bss).
SYSROOT="$(rustc --print sysroot)"
LLVM_SIZE="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin/llvm-size"
if [ ! -x "$LLVM_SIZE" ]; then
    echo "boot-test: KHÔNG đo được .data+.bss (thiếu llvm-size ở $LLVM_SIZE) — K0.5 chưa đạt" >&2
    exit 1
fi
MB="$("$LLVM_SIZE" -A "$IMAGE" 2>/dev/null | awk '
    $1==".data"||$1==".sdata"||$1==".bss"||$1==".sbss" {s+=$2}
    END {print s+0}')"
STACK="$("$LLVM_SIZE" -A "$IMAGE" 2>/dev/null | awk '$1==".stack" {print $2+0; exit}')"
IMG="$(wc -c <"$IMAGE" | tr -d ' ')"
echo "# image = $IMG byte | .data+.bss = ${MB:-?} byte (trần $BUDGET_BYTES) | .stack = ${STACK:-0} byte (báo riêng, NOLOAD)"
if [ "${MB:-0}" -gt "$BUDGET_BYTES" ]; then
    echo "boot-test: VƯỢT cổng cứng ${MB} > ${BUDGET_BYTES} byte — KHÔNG đạt K0.5" >&2
    exit 1
fi

fails=0
i=1
while [ "$i" -le "$N" ]; do
    out="/tmp/v2-kbin-boot-$i.log"
    "$QEMU" -machine virt -bios "$SBI" -kernel "$IMAGE" -nographic -smp 1 -m 128M \
        -no-reboot >"$out" 2>&1 &
    pid=$!
    # Không treo: chờ tối đa 10 s rồi kill ĐÚNG pid do task này tạo (AGENTS.md §9).
    waited=0
    while kill -0 "$pid" 2>/dev/null && [ "$waited" -lt 100 ]; do
        sleep 0.1
        waited=$((waited + 1))
    done
    if kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null
        wait "$pid" 2>/dev/null
        echo "not ok $i - boot $i: quá 10 s (treo), log $out"
        fails=$((fails + 1))
        i=$((i + 1))
        continue
    fi
    wait "$pid" 2>/dev/null
    if grep -q 'TAP version 13' "$out" && grep -q '^1\.\.1$' "$out" &&
        grep -q '^ok 1 - k0\.5 boot$' "$out" && grep -q '^# k0\.5 halt$' "$out"; then
        echo "ok $i - boot $i"
    else
        echo "not ok $i - boot $i: thiếu marker TAP/halt, log $out"
        fails=$((fails + 1))
    fi
    i=$((i + 1))
done

echo "# $((N - fails))/$N boot sạch"
if [ "$fails" -ne 0 ]; then
    exit 1
fi
echo "# boot-test: PASS"
