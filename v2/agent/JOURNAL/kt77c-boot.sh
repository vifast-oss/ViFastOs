#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-77c — boot ảnh **sản phẩm** (KT-77c: U-mode + shell + FS + virtio-blk) trên QEMU riscv64,
# tuỳ chọn gắn ảnh đĩa qua `-drive`/`-device virtio-blk-pci`, bơm kịch bản serial từ **ngoài**.
#
# Dùng: kt77c-boot.sh <image> <disk|-> <smp> <payload|-> <log> [timeout_ds]
#   <disk>  = tệp ảnh đĩa RAW (cùng tệp cho nhiều lần boot ⇒ đo đọc-lại-sau-reboot), '-' = không đĩa
#   <payload> = tệp byte bơm vào stdin QEMU qua `stdin_wrap.sh` của KT-77a, '-' = không bơm
#   timeout_ds = bước 0,1 s (mặc định 300 = 30 s, > watchdog 20 s của harness)
#
# D2-116: mọi boot đi qua `v2/test/boot/qemu_guard.sh` **không sửa** (process group riêng, timeout
# cứng, dọn khi cha chết) + `qg_infra` trước/sau + đếm QEMU còn sót. Không `pkill` toàn cục.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
ROOT="$(cd "$V2_ROOT/.." && pwd)"
GUARD="$V2_ROOT/test/boot/qemu_guard.sh"
WRAP="$V2_ROOT/test/parity/stdin_wrap.sh"

IMAGE="${1:?cần <image>}"
DISK="${2:--}"
SMP="${3:-1}"
PAYLOAD="${4:--}"
LOG="${5:?cần <log>}"
TIMEOUT_DS="${6:-300}"

[ -x "$IMAGE" ] || { echo "kt77c-boot: không thấy ảnh $IMAGE" >&2; exit 2; }
[ -f "$GUARD" ] || { echo "kt77c-boot: không thấy $GUARD" >&2; exit 2; }

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"
QEMU="$(command -v qemu-system-riscv64 || true)"
[ -n "$QEMU" ] || { echo "kt77c-boot: không có qemu-system-riscv64" >&2; exit 2; }
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
[ -f "${SBI:-/nonexistent}" ] || { echo "kt77c-boot: không thấy OpenSBI fw_dynamic" >&2; exit 2; }

mkdir -p "$(dirname "$LOG")"
IMG_SHA="$(shasum -a 256 "$IMAGE" | awk '{print $1}')"
IMG_BYTES="$(wc -c <"$IMAGE" | tr -d ' ')"
DISK_SHA="-"
[ "$DISK" != "-" ] && DISK_SHA="$(shasum -a 256 "$DISK" 2>/dev/null | awk '{print $1}')"

QARGS=(-machine virt,aia=none,aclint=on -bios "$SBI" -kernel "$IMAGE" -nographic \
    -cpu rv64 -m 128M -smp "$SMP" -no-reboot)
if [ "$DISK" != "-" ]; then
    QARGS+=(-drive "file=$DISK,if=none,format=raw,id=hd0" -device virtio-blk-pci,drive=hd0)
fi

. "$GUARD"
QG_DIR="${TMPDIR:-/tmp}"
qg_init || { echo "kt77c-boot: qemu_guard init FAIL" >&2; exit 2; }
INFRA="$(dirname "$LOG")/kt77c-infra.log"
qg_infra before "$INFRA" >/dev/null 2>&1 || true

{
    echo "# kt77c-boot: image=$IMAGE"
    echo "# kt77c-boot: image_sha256=$IMG_SHA image_bytes=$IMG_BYTES"
    echo "# kt77c-boot: disk=${DISK} disk_sha256=${DISK_SHA} smp=$SMP payload=${PAYLOAD} timeout_ds=$TIMEOUT_DS"
    echo "# kt77c-boot: qemu=$QEMU opensbi=$SBI guard=$GUARD"
} >"$LOG.pre"

RC=0
if [ "$PAYLOAD" = "-" ]; then
    qg_boot "$LOG.body" "$TIMEOUT_DS" "$QEMU" "${QARGS[@]}"
    RC=$?
else
    KT77_SEND_DELAY="${KT77_SEND_DELAY:-0.05}" \
        qg_boot "$LOG.body" "$TIMEOUT_DS" "$WRAP" "$PAYLOAD" "$QEMU" "${QARGS[@]}"
    RC=$?
fi
{
    cat "$LOG.pre"
    echo "# kt77c-boot: qemu_rc=$RC"
    cat "$LOG.body" 2>/dev/null || true
} >"$LOG"
rm -f "$LOG.pre" "$LOG.body" 2>/dev/null

qg_infra after "$INFRA" >/dev/null 2>&1 || true
own="$(qg_own)"
live="$(ps -ax -o pid,command 2>/dev/null | grep -c '[q]emu-system' || true)"
echo "# kt77c-boot: qemu_con_sot_total=${live:-0} qemu_cua_script=${own:-0} log=$LOG" >>"$LOG"
exit "$RC"
