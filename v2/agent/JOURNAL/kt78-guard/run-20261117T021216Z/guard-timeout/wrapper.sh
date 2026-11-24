#!/usr/bin/env bash
# KT-78: process group riêng + trap + timeout cứng + watchdog (kind=hang)
set -u
. /Users/mps/dvchd/ViFastOs/v2/test/boot/qemu_guard.sh
QG_DIR=/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/run-20261117T021216Z/guard-timeout/qgdir
QG_LOG=/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/run-20261117T021216Z/guard-timeout/qg.log
LOGFILE=/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/run-20261117T021216Z/guard-timeout/qemu.log
TDS=20
QEMU=/opt/homebrew/bin/qemu-system-riscv64
HANG=/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/run-20261117T021216Z/guard-timeout/hang.bin
INFOFILE=/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/run-20261117T021216Z/guard-timeout/info.txt
qg_init || exit 2
printf '# wrapper: pid=%s pgid=%s\n' "$$" "$(ps -o pgid= -p $$ | tr -d ' ')" >"$INFOFILE"
qg_boot "$LOGFILE" "$TDS" "$QEMU" -bios "$HANG" -nographic -machine virt,aia=none,aclint=on \
    -cpu rv64 -m 128M -smp 1 -no-reboot
exit $?
