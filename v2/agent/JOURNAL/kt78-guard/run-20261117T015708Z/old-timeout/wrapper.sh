#!/usr/bin/env bash
# pattern CŨ + khách treo — đo nhánh timeout (đối chứng: nhánh này vốn đã kill được con trực tiếp)
set -u
QEMU="/opt/homebrew/bin/qemu-system-riscv64"
"$QEMU" -bios "/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/run-20261117T015708Z/old-timeout/hang.bin" -nographic -machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp 1 -no-reboot >"/Users/mps/dvchd/ViFastOs/v2/agent/JOURNAL/kt78-guard/run-20261117T015708Z/old-timeout/qemu.log" 2>&1 &
pid=$!
waited=0
while kill -0 "$pid" 2>/dev/null && [ "$waited" -lt "20" ]; do
    sleep 0.1
    waited=$((waited + 1))
done
if kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null
    wait "$pid" 2>/dev/null
    exit 124
fi
wait "$pid" 2>/dev/null
exit 0
