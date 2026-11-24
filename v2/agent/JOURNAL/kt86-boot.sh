#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-86 — boot MỘT ảnh riscv64 qua `v2/test/boot/qemu_guard.sh` (process group riêng, timeout kill
# cả group, chỉ `kill` PID do chính script này tạo — AGENTS §9/D2-116), ghi log serial.
#
# Dùng cho bằng chứng KT-86 (dòng `# build:` + cổng danh tính):
#   bash v2/agent/JOURNAL/kt86-boot.sh <ảnh> <log> [<smp>] [tham số QEMU thêm...]
# In dòng cuối `BOOT: rc=<mã> pid=<pid> secs=<giây> log=<log>` để log của ca kiểm grep được.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

IMAGE="${1:-}"
LOG="${2:-}"
SMP="${3:-1}"
shift 3 2>/dev/null || shift 2 2>/dev/null || true

die() {
    echo "kt86-boot: $*" >&2
    exit 2
}
[ -f "$IMAGE" ] || die "không thấy ảnh [$IMAGE]"
[ -n "$LOG" ] || die "cần đường dẫn log"

mkdir -p "$(dirname "$LOG")" || die "không tạo được thư mục log $(dirname "$LOG")"
LOGDIR="$(cd "$(dirname "$LOG")" && pwd)"

. "$V2_ROOT/test/boot/qemu_guard.sh"
QG_DIR="${TMPDIR:-/tmp}"
QG_LOG="${KT86_QG_LOG:-$LOGDIR/qemu-guard.log}"
qg_init || die "qemu_guard: không khởi tạo được (thiếu perl?) — KT-78"

QEMU="$(command -v qemu-system-riscv64 || true)"
[ -n "$QEMU" ] || die "không có qemu-system-riscv64"
SBI="${OPENSBI:-}"
if [ -z "$SBI" ]; then
    for c in /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
        /usr/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
        /usr/local/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin; do
        [ -f "$c" ] && {
            SBI="$c"
            break
        }
    done
fi
[ -f "${SBI:-/nonexistent}" ] || die "không tìm thấy OpenSBI fw_dynamic (đặt OPENSBI=...)"

qg_boot "$LOG" 200 "$QEMU" -machine virt,aia=none,aclint=on -bios "$SBI" \
    -kernel "$IMAGE" -nographic -cpu rv64 -m 128M -smp "$SMP" -no-reboot "$@" >/dev/null 2>&1
rc=$?
echo "BOOT: rc=$rc pid=${QG_LAST_PID:-} secs=${QG_LAST_SECS:-} smp=$SMP image=$IMAGE log=$LOG"
exit 0
