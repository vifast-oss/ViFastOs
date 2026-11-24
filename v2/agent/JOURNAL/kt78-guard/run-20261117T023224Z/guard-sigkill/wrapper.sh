#!/usr/bin/env bash
# KT-78: process group riêng + trap + timeout cứng + watchdog (kind=S)
set -u
. /Users/mps/dvchd/ViFastOs/v2/test/boot/qemu_guard.sh
QG_DIR=/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/run-20261117T023224Z/guard-sigkill/qgdir
QG_LOG=/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/run-20261117T023224Z/guard-sigkill/qg.log
LOGFILE=/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/run-20261117T023224Z/guard-sigkill/qemu.log
TDS=600
QEMU=/opt/homebrew/bin/qemu-system-riscv64
IMAGE=/Users/mps/dvchd/ViFastOs/v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
INFOFILE=/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/run-20261117T023224Z/guard-sigkill/info.txt
qg_init || exit 2
printf '# wrapper: pid=%s pgid=%s\n' "$$" "$(ps -o pgid= -p $$ | tr -d ' ')" >"$INFOFILE"
qg_boot "$LOGFILE" "$TDS" "$QEMU" -S -kernel "$IMAGE" -nographic -machine virt,aia=none,aclint=on \
    -cpu rv64 -m 128M -smp 1 -no-reboot
exit $?
