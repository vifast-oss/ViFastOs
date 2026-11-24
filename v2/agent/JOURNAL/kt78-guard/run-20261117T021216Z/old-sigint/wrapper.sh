#!/usr/bin/env bash
# pattern CŨ (trước KT-78) — tái hiện D2-116
set -u
QEMU="/opt/homebrew/bin/qemu-system-riscv64"
"$QEMU" -S -nographic -machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp 1 -no-reboot >"/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/run-20261117T021216Z/old-sigint/qemu.log" 2>&1 &
pid=$!
printf '# wrapper_cu: pid=%s pgid=%s qemu=%s\n' "$$" "$(ps -o pgid= -p "$$" | tr -d ' ')" "$pid" >"/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/run-20261117T021216Z/old-sigint/info.txt"
: >"/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/run-20261117T021216Z/old-sigint/ready"
sleep 600 & wait $!
