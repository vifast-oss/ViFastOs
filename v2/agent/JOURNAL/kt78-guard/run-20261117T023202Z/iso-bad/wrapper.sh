#!/usr/bin/env bash
# KT-78: process group riêng + trap + timeout cứng + watchdog (kind=iso_bad)
set -u
. /Users/mps/dvchd/ViFastOs/v2/test/boot/qemu_guard.sh
QG_DIR=/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/run-20261117T023202Z/iso-bad/qgdir
QG_LOG=/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/run-20261117T023202Z/iso-bad/qg.log
LOGFILE=/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/run-20261117T023202Z/iso-bad/qemu.log
TDS=200
QEMU=/opt/homebrew/bin/qemu-system-riscv64
IMAGE=/Users/mps/dvchd/ViFastOs/v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
INFOFILE=/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/run-20261117T023202Z/iso-bad/info.txt
QG_FORCE_NO_SETPGRP=${QG_FORCE_NO_SETPGRP:-1}
qg_init || exit 2
printf '# wrapper: pid=%s pgid=%s\n' "$$" "$(ps -o pgid= -p $$ | tr -d ' ')" >"$INFOFILE"
qg_boot "$LOGFILE" "$TDS" "$QEMU" -kernel "$IMAGE" -nographic -machine virt,aia=none,aclint=on \
    -cpu rv64 -m 128M -smp 1 -no-reboot
rc=$?
printf "# iso_bad rc=%s isolation_bad=%s\n" "$rc" "$QG_ISO_BAD" >"$INFOFILE"
qg_infra after-iso-bad >>"$INFOFILE" 2>&1 || true
exit $rc
