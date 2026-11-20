#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-251 — xác minh **S1** trên **ảnh sản phẩm** (`product-shell,product-fs`): boot QEMU `virt`
# smp 1 và 2, bơm `help` / `ls /` / `cat /README` / `exit` qua serial (UART → ring → shell U-mode),
# lưu log serial **nguyên văn** vào `v2/agent/JOURNAL/kt251-s1-smp<N>.log`.
#
# Không build ở đây: ảnh do `v2/tools/repro-build.sh` dựng (đường canonical) và truyền qua
# `KT251_IMAGE`; script chỉ **đo**. Dùng `qemu_guard.sh` (KT-78) + `stdin_wrap.sh` (KT-77a) —
# cùng khung với `kt192-closure-measure.sh` §F1, không tự viết lại vòng đời QEMU.
#
# Dùng: KT251_IMAGE=<ảnh> bash v2/agent/JOURNAL/kt251-s1-measure.sh
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT" || exit 1
LOGDIR="${KT251_LOGDIR:-$ROOT/v2/agent/JOURNAL}"
IMAGE="${KT251_IMAGE:?dat KT251_IMAGE=<duong dan anh san pham>}"
[ -f "$IMAGE" ] || {
    echo "kt251: không thấy ảnh $IMAGE" >&2
    exit 2
}
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"
SBI="${OPENSBI:-/opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin}"
[ -f "$SBI" ] || {
    echo "kt251: không thấy OpenSBI $SBI" >&2
    exit 2
}
QEMU="$(command -v qemu-system-riscv64)" || {
    echo "kt251: không có qemu-system-riscv64" >&2
    exit 2
}
. "$ROOT/v2/test/boot/qemu_guard.sh"
. "$ROOT/v2/test/boot/build-rev.sh"
QG_DIR="${TMPDIR:-/tmp}"
QG_LOG="$LOGDIR/${KT251_PREFIX:-kt251}-qemu-guard.log"
rm -f "$QG_LOG" 2>/dev/null
qg_init || {
    echo "kt251: qg_init FAIL" >&2
    exit 2
}

echo "### KT-251 S1 $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "### image=$IMAGE"
echo "### image_sha256=$(shasum -a 256 "$IMAGE" | awk '{print $1}')"
echo "### image_bytes=$(wc -c <"$IMAGE" | tr -d ' ')"
echo "### head=$(git rev-parse HEAD)"
echo "### qemu=$("$QEMU" --version | head -1)"
echo "### opensbi=$SBI"

for SMP in 1 2; do
    L="$LOGDIR/${KT251_PREFIX:-kt251}-s1-smp$SMP.log"
    PAY="$LOGDIR/.kt251-pay-$SMP"
    printf 'help\nls /\ncat /README\nexit\n' >"$PAY"
    KT77_SEND_DELAY="${KT251_SEND_DELAY:-0.5}" qg_boot "$L" 300 "$ROOT/v2/test/parity/stdin_wrap.sh" "$PAY" \
        "$QEMU" -machine virt,aia=none,aclint=on -bios "$SBI" \
        -kernel "$IMAGE" -nographic -cpu rv64 -m 128M -smp "$SMP" -no-reboot
    echo "### boot smp=$SMP qemu_rc=$? secs=$QG_LAST_SECS log=$L"
    # Chờ vòng bơm của `stdin_wrap.sh` (0,05 s + 8 × 0,02 s) kết thúc rồi mới xoá payload —
    # nếu xoá ngay, subshell bơm còn chạy sẽ in `cat: … No such file` vào log serial.
    sleep 0.5
    rm -f "$PAY"
    grep -a -m1 -E '^# build:' "$L"
    grep -a -m1 -E '^# product shell:' "$L"
    echo "### smp=$SMP shell_prompt_lines=$(grep -a -c '^shell>' "$L")"
    echo "### smp=$SMP help_cmds=$(grep -a -o -m1 -E 'help cmds=[0-9]+' "$L" | head -1)"
    echo "### smp=$SMP ls_ok=$(grep -a -c -E '^# shell: ls n=[0-9]+ path=/$' "$L")"
    echo "### smp=$SMP cat_ok=$(grep -a -c -E '^# shell: cat ok bytes=[0-9]+ first=' "$L")"
    echo "### smp=$SMP exit_ok=$(grep -a -c -E '^# shell: exit code=0$' "$L")"
    vbrev_gate "$L" "$ROOT/v2" "$IMAGE" || echo "### REV-GATE REFUSE smp=$SMP"
done
