#!/usr/bin/env bash
# KT-205 — probe tay: biên MAX_PATH 48 B + `cd` có `/` cuối. Chỉ dùng cho bằng chứng;
# KHÔNG phải harness (harness là v2/test/boot/shell-edge.sh).
set -u
IMAGE="${1:?cần ảnh}"
LOG="${2:-/tmp/kt205-probe-qemu.log}"
FIFO="$(mktemp -u /tmp/kt205-probe.XXXXXX.fifo)"
rm -f "$LOG" "$FIFO"
mkfifo "$FIFO" || exit 1
exec 8<>"$FIFO" || exit 1
qemu-system-riscv64 -machine virt,aia=none,aclint=on \
    -bios /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin \
    -kernel "$IMAGE" -nographic -cpu rv64 -m 128M -smp 1 -no-reboot \
    <"$FIFO" >"$LOG" 2>&1 &
QPID=$!
i=0
while [ "$i" -lt 300 ]; do
    grep -q '^shell>$' "$LOG" 2>/dev/null && break
    kill -0 "$QPID" 2>/dev/null || break
    sleep 0.1
    i=$((i + 1))
done
send() { printf '%s\n' "$1" >&8 2>/dev/null || true; sleep 0.25; }
a38="$(printf 'a%.0s' $(seq 1 38))"
a39="$(printf 'a%.0s' $(seq 1 39))"
p48="/docs/sub/$a38"
p49="/docs/sub/$a39"
echo "# probe: len(p48)=${#p48} len(p49)=${#p49}" >&2
send 'cd /docs/sub'
send "cd $p48"
send "cd $p49"
send 'pwd'
send 'cd /docs/sub/'
send 'pwd'
send 'exit'
i=0
while [ "$i" -lt 200 ] && kill -0 "$QPID" 2>/dev/null; do
    sleep 0.1
    i=$((i + 1))
done
kill "$QPID" 2>/dev/null || true
wait "$QPID" 2>/dev/null || true
exec 8>&-
rm -f "$FIFO"
cat "$LOG"
