#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-44 — boot ảnh **TEST** riscv64 K3 (bin `vifast-kbin-ktest`, feature `ktest`).
#
# Đây là phép **đỏ trước** của KT-44: script đòi dòng TAP `ok 1 - ktest image`; khi chưa có
# ảnh test (chưa có `src/ktest.rs`/feature) nó ĐỎ. Sau khi tách ảnh thì xanh.
#
# D2-08: ảnh test KHÔNG nằm trong ảnh sản phẩm — cùng crate nhưng bin + feature riêng.
# D2-72 Q2/Q5/Q10: `-machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp N`, budget riêng
# cho ảnh test, timeout 20 s/boot, chỉ `kill <pid>` do chính script này tạo (AGENTS §9).
#
# Dùng: bash v2/test/boot/ktest.sh [NBOOT] [SMP]   (mặc định 1 boot, 1 hart)
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
BUDGET="$HERE/budget.sh"
TARGET="riscv64gc-unknown-none-elf"
IMAGE="$V2_ROOT/kernel/target/$TARGET/release/vifast-kbin-ktest"
N_BOOT="${1:-1}"
SMP="${2:-1}"
TIMEOUT_DS=200   # 20 s/boot (D2-72 Q10), đơn vị 0,1 s
QEMU_MIN_MAJOR=9 # sàn D2-72 Q2

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

die() { echo "ktest.sh: $*" >&2; exit 1; }

command -v cargo >/dev/null 2>&1 || die "không có cargo"
QEMU="$(command -v qemu-system-riscv64 || true)"
[ -n "$QEMU" ] || die "không có qemu-system-riscv64"

# OpenSBI `fw_dynamic` nạp trực tiếp, không U-Boot/EDK2 (D2-72 Q3).
SBI="${OPENSBI:-}"
if [ -z "$SBI" ]; then
    for c in /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
        /usr/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
        /usr/local/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin; do
        [ -f "$c" ] && { SBI="$c"; break; }
    done
fi
[ -f "${SBI:-/nonexistent}" ] || die "không tìm thấy OpenSBI fw_dynamic (đặt OPENSBI=...)"

# QEMU version: thiếu hoặc major dưới sàn ⇒ FAIL, không tự hạ (D2-72 Q2).
QEMU_VERSION="$("$QEMU" --version 2>/dev/null | sed -n '1s/.*version \([0-9][^ ]*\).*/\1/p')"
[ -n "$QEMU_VERSION" ] || die "không đọc được phiên bản QEMU"
QEMU_MAJOR="${QEMU_VERSION%%.*}"
[ "$QEMU_MAJOR" -ge "$QEMU_MIN_MAJOR" ] || die "QEMU $QEMU_VERSION < sàn $QEMU_MIN_MAJOR.x (D2-72 Q2) — không tự hạ"
echo "# qemu $QEMU_VERSION (sàn $QEMU_MIN_MAJOR.x, D2-72 Q2)"
echo "# opensbi $SBI"

echo "# build ảnh test ($TARGET, release, --features ktest)"
if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-riscv64 --bin vifast-kbin-ktest \
    --features ktest --target "$TARGET" --release >/tmp/k3-ktest-build.log 2>&1; then
    tail -20 /tmp/k3-ktest-build.log >&2
    die "build ảnh test FAIL — CHƯA có ảnh test (xem /tmp/k3-ktest-build.log)"
fi
[ -f "$IMAGE" ] || die "không thấy ảnh test $IMAGE — CHƯA có ảnh test"

# Budget: ảnh test cũng phải ≤ 262.144 B (D2-72 Q5, không tách ngân sách riêng).
bash "$BUDGET" "$IMAGE" ktest || die "budget ảnh test VƯỢT trần"

# Ảnh test phải mang chuỗi `ktest`; ảnh sản phẩm thì không (D2-08).
if command -v strings >/dev/null 2>&1; then
    HITS="$(strings "$IMAGE" 2>/dev/null | grep -c ktest || true)"
else
    HITS="$(LC_ALL=C grep -a -o ktest "$IMAGE" 2>/dev/null | wc -l | tr -d ' ' || true)"
fi
[ "${HITS:-0}" -ge 1 ] || die "ảnh test không chứa chuỗi 'ktest' — nhầm ảnh?"
echo "# image=ktest ktest-strings=$HITS"

echo "# boot: n=$N_BOOT smp=$SMP timeout=$((TIMEOUT_DS / 10))s"
fails=0
i=1
while [ "$i" -le "$N_BOOT" ]; do
    out="/tmp/k3-ktest-boot-$i.log"
    "$QEMU" -machine virt,aia=none,aclint=on -bios "$SBI" -kernel "$IMAGE" -nographic \
        -cpu rv64 -m 128M -smp "$SMP" -no-reboot >"$out" 2>&1 &
    pid=$!
    waited=0
    while kill -0 "$pid" 2>/dev/null && [ "$waited" -lt "$TIMEOUT_DS" ]; do
        sleep 0.1
        waited=$((waited + 1))
    done
    if kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null # chỉ kill ĐÚNG pid do script này tạo (AGENTS §9)
        wait "$pid" 2>/dev/null
        echo "not ok $i - ktest boot $i: quá $((TIMEOUT_DS / 10)) s (treo), log $out"
        fails=$((fails + 1))
        i=$((i + 1))
        continue
    fi
    wait "$pid" 2>/dev/null
    if grep -q '^OpenSBI v' "$out"; then
        echo "# opensbi $(sed -n 's/^OpenSBI \(v[0-9][0-9.]*\).*/\1/p' "$out" | head -1)"
    fi
    if grep -q '^TAP version 13$' "$out" && grep -q '^1\.\.1$' "$out" &&
        grep -q '^ok 1 - ktest image$' "$out" && grep -q '^# ktest halt$' "$out" &&
        ! grep -q '^PANIC' "$out"; then
        echo "ok $i - ktest image"
    else
        echo "not ok $i - ktest boot $i: thiếu 'ok 1 - ktest image' hoặc có PANIC, log $out"
        fails=$((fails + 1))
    fi
    i=$((i + 1))
done

echo "# $((N_BOOT - fails))/$N_BOOT boot ảnh test sạch (smp=$SMP)"
[ "$fails" -eq 0 ] || exit 1
echo "# ktest.sh: PASS"
