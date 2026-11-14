#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-44/45/46 — boot ảnh **TEST** riscv64 K3 (crate `vifast-kbin-riscv64`, feature `ktest`).
#
# D2-08: ảnh test KHÔNG nằm trong ảnh sản phẩm — cùng crate nhưng bin + feature riêng.
# KT-46: **hai** ảnh test vì mỗi boot chỉ kết thúc được một lần bằng PANIC có chủ đích:
#   - `vifast-kbin-ktest`       : 8 TAP — KT-45 (image + 3 trap + nested) + KT-46 (mmu on, map
#                                 layout, walk khop) trước trap lồng fatal cuối cùng.
#   - `vifast-kbin-ktest-guard` : 3 TAP — guard page fault **thật**: load `scause=13` (resume
#                                 probe) rồi store `scause=15` fatal `PANIC: guard page fault`.
#
# D2-72 Q2/Q5/Q10: `-machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp N`, budget riêng
# cho **mỗi** ảnh test (≤ 262.144 B), timeout 20 s/boot, chỉ `kill <pid>` do chính script tạo
# (AGENTS §9).
#
# Dùng: bash v2/test/boot/ktest.sh [NBOOT] [SMP]   (mặc định 1 boot, 1 hart)
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
MANIFEST="$V2_ROOT/kernel/Cargo.toml"
BUDGET="$HERE/budget.sh"
TARGET="riscv64gc-unknown-none-elf"
IMG_MAIN="$V2_ROOT/kernel/target/$TARGET/release/vifast-kbin-ktest"
IMG_GUARD="$V2_ROOT/kernel/target/$TARGET/release/vifast-kbin-ktest-guard"
N_BOOT="${1:-1}"
SMP="${2:-1}"
TIMEOUT_DS=200   # 20 s/boot (D2-72 Q10), đơn vị 0,1 s
QEMU_MIN_MAJOR=9 # sàn D2-72 Q2: QEMU >= 9.2
QEMU_MIN_MINOR=2

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

# QEMU version: thiếu hoặc dưới sàn major.minor ⇒ FAIL, không tự hạ (D2-72 Q2).
QEMU_VERSION="$("$QEMU" --version 2>/dev/null | sed -n '1s/.*version \([0-9][^ ]*\).*/\1/p')"
[ -n "$QEMU_VERSION" ] || die "không đọc được phiên bản QEMU"
case "$QEMU_VERSION" in *.*) ;; *) die "phiên bản QEMU '$QEMU_VERSION' không có dạng major.minor" ;; esac
QEMU_MAJOR="${QEMU_VERSION%%.*}"
QEMU_REST="${QEMU_VERSION#*.}"
QEMU_MINOR="${QEMU_REST%%.*}"
case "$QEMU_MAJOR" in '' | *[!0-9]*) die "không đọc được major của QEMU '$QEMU_VERSION'" ;; esac
case "$QEMU_MINOR" in '' | *[!0-9]*) die "không đọc được minor của QEMU '$QEMU_VERSION'" ;; esac
if [ "$QEMU_MAJOR" -lt "$QEMU_MIN_MAJOR" ] ||
    { [ "$QEMU_MAJOR" -eq "$QEMU_MIN_MAJOR" ] && [ "$QEMU_MINOR" -lt "$QEMU_MIN_MINOR" ]; }; then
    die "QEMU $QEMU_VERSION < sàn $QEMU_MIN_MAJOR.$QEMU_MIN_MINOR (D2-72 Q2) — không tự hạ"
fi
echo "# qemu $QEMU_VERSION (sàn $QEMU_MIN_MAJOR.$QEMU_MIN_MINOR, D2-72 Q2)"
echo "# opensbi $SBI"

echo "# build 2 ảnh test ($TARGET, release, --features ktest)"
if ! cargo build --manifest-path "$MANIFEST" -p vifast-kbin-riscv64 \
    --bin vifast-kbin-ktest --bin vifast-kbin-ktest-guard \
    --features ktest --target "$TARGET" --release >/tmp/k3-ktest-build.log 2>&1; then
    tail -20 /tmp/k3-ktest-build.log >&2
    die "build ảnh test FAIL — CHƯA có ảnh test (xem /tmp/k3-ktest-build.log)"
fi
for IMAGE in "$IMG_MAIN" "$IMG_GUARD"; do
    [ -f "$IMAGE" ] || die "không thấy ảnh test $IMAGE — CHƯA có ảnh test"
done

# Budget: mỗi ảnh test cũng phải ≤ 262.144 B (D2-72 Q5, không tách ngân sách riêng).
bash "$BUDGET" "$IMG_MAIN" ktest || die "budget ảnh test VƯỢT trần"
bash "$BUDGET" "$IMG_GUARD" ktest-guard || die "budget ảnh test guard VƯỢT trần"

# Ảnh test phải mang chuỗi `ktest` (D2-08: tách khỏi ảnh sản phẩm).
for PAIR in "ktest:$IMG_MAIN" "ktest-guard:$IMG_GUARD"; do
    LABEL="${PAIR%%:*}"
    IMAGE="${PAIR#*:}"
    if command -v strings >/dev/null 2>&1; then
        HITS="$(strings "$IMAGE" 2>/dev/null | grep -c ktest || true)"
    else
        HITS="$(LC_ALL=C grep -a -o ktest "$IMAGE" 2>/dev/null | wc -l | tr -d ' ' || true)"
    fi
    [ "${HITS:-0}" -ge 1 ] || die "ảnh $LABEL không chứa chuỗi 'ktest' — nhầm ảnh?"
    echo "# image=$LABEL ktest-strings=$HITS"
done

# boot_once <tag> <image> <index>: boot QEMU, in đường dẫn log qua stdout (chỉ đường dẫn).
boot_once() {
    local tag="$1" image="$2" index="$3"
    local out="/tmp/k3-ktest-$tag-$index.log"
    "$QEMU" -machine virt,aia=none,aclint=on -bios "$SBI" -kernel "$image" -nographic \
        -cpu rv64 -m 128M -smp "$SMP" -no-reboot >"$out" 2>&1 &
    local pid=$!
    local waited=0
    while kill -0 "$pid" 2>/dev/null && [ "$waited" -lt "$TIMEOUT_DS" ]; do
        sleep 0.1
        waited=$((waited + 1))
    done
    if kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null # chỉ kill ĐÚNG pid do script này tạo (AGENTS §9)
        wait "$pid" 2>/dev/null
        echo "$out"
        return 124
    fi
    wait "$pid" 2>/dev/null
    if grep -q '^OpenSBI v' "$out"; then
        echo "# opensbi $(sed -n 's/^OpenSBI \(v[0-9][0-9.]*\).*/\1/p' "$out" | head -1)" >&2
    fi
    echo "$out"
    return 0
}

fails=0
i=1
while [ "$i" -le "$N_BOOT" ]; do
    # ---------------------------------------------------------------- ảnh chính: 8 TAP
    if out="$(boot_once main "$IMG_MAIN" "$i")"; then :; else
        echo "not ok $i - ktest boot $i: quá $((TIMEOUT_DS / 10)) s (treo), log $out"
        fails=$((fails + 1))
        i=$((i + 1))
        continue
    fi
    # TAP 1..4 là KT-45; 5..7 là KT-46 (MMU); 8 là trap lồng fatal có chủ đích. Runner đòi
    # đúng dòng PANIC lồng và **chỉ** dòng đó.
    missing=""
    grep -q '^TAP version 13$' "$out" || missing="$missing TAP-version"
    grep -q '^1\.\.8$' "$out" || missing="$missing plan-1..8"
    grep -q '^ok 1 - ktest image$' "$out" || missing="$missing ok1-image"
    grep -q '^ok 2 - trap breakpoint$' "$out" || missing="$missing ok2-breakpoint"
    grep -q '^ok 3 - trap ecall sbi$' "$out" || missing="$missing ok3-ecall-sbi"
    grep -q '^ok 4 - trap illegal inject$' "$out" || missing="$missing ok4-illegal-inject"
    grep -q '^ok 5 - mmu on$' "$out" || missing="$missing ok5-mmu-on"
    grep -q '^ok 6 - map layout$' "$out" || missing="$missing ok6-map-layout"
    grep -q '^ok 7 - walk khop$' "$out" || missing="$missing ok7-walk-khop"
    grep -q '^ok 8 - trap nested panic$' "$out" || missing="$missing ok8-nested"
    grep -q '^# mmu: .*mode=8' "$out" || missing="$missing mmu-evidence"
    grep -q '^# mmu on: .*alias=1 .*kernel_low=1 kernel_high=1' "$out" ||
        missing="$missing mmu-on-evidence"
    grep -q '^# mmu layout: .*none_low=1 none_high=1 none_physmap=1' "$out" ||
        missing="$missing guard-none-evidence"
    grep -q '^# mmu layout: .*physmap_2m_nx=1 uart_dev=1 plic_dev=1 kernel_high_2m=1' "$out" ||
        missing="$missing device/physmap-evidence"
    grep -q '^# walk khop: pre=post=1 software=8/8' "$out" ||
        missing="$missing walk-khop-evidence"
    grep -q '^PANIC: nested trap (KT-45: trap trong handler = fatal, khong resume)$' "$out" ||
        missing="$missing nested-panic"
    others="$(grep '^PANIC' "$out" |
        grep -vc '^PANIC: nested trap (KT-45: trap trong handler = fatal, khong resume)$' || true)"
    [ "${others:-0}" -eq 0 ] || missing="$missing PANIC-khác"
    if [ -z "$missing" ]; then
        echo "ok $i - ktest image + trap + mmu + nested"
    else
        echo "not ok $i - ktest boot $i: thiếu$missing, log $out"
        fails=$((fails + 1))
    fi

    # ------------------------------------------------------- ảnh guard: 3 TAP, store fatal
    if out="$(boot_once guard "$IMG_GUARD" "$i")"; then :; else
        echo "not ok $i - guard ktest boot $i: quá $((TIMEOUT_DS / 10)) s (treo), log $out"
        fails=$((fails + 1))
        i=$((i + 1))
        continue
    fi
    missing=""
    grep -q '^TAP version 13$' "$out" || missing="$missing TAP-version"
    grep -q '^1\.\.3$' "$out" || missing="$missing plan-1..3"
    grep -q '^ok 1 - guard image$' "$out" || missing="$missing ok1-guard-image"
    grep -q '^ok 2 - guard load fault$' "$out" || missing="$missing ok2-guard-load"
    grep -q '^ok 3 - guard store fault$' "$out" || missing="$missing ok3-guard-store"
    grep -q '^# guard fault load scause=0xd stval=0x[0-9a-f]* sepc=0x[0-9a-f]* len=[24] recovered=1$' "$out" ||
        missing="$missing guard-load-real"
    grep -q '^# guard load stats: .*last_scause=0xd' "$out" ||
        missing="$missing guard-load-stats"
    grep -q '^PANIC: guard page fault scause=0xf stval=0x[0-9a-f]* sepc=0x[0-9a-f]*' "$out" ||
        missing="$missing guard-store-panic"
    others="$(grep '^PANIC' "$out" |
        grep -vc '^PANIC: guard page fault scause=0xf ' || true)"
    [ "${others:-0}" -eq 0 ] || missing="$missing PANIC-khác"
    if [ -z "$missing" ]; then
        echo "ok $i - guard image + load 13 + store 15"
    else
        echo "not ok $i - guard ktest boot $i: thiếu$missing, log $out"
        fails=$((fails + 1))
    fi
    i=$((i + 1))
done

echo "# $((N_BOOT * 2 - fails))/$((N_BOOT * 2)) boot ảnh test sạch (smp=$SMP)"
[ "$fails" -eq 0 ] || exit 1
echo "# ktest.sh: PASS"
