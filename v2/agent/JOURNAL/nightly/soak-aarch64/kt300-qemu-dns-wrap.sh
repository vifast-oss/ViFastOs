#!/bin/bash
# KT-300/KT-406 — chỉ soak: đặt lại biến nạp (DYLD_INSERT_LIBRARIES trên macOS, LD_PRELOAD trên
# Linux) rồi exec QEMU thật.
export "DYLD_INSERT_LIBRARIES=/Users/mps/dvchd/ViFastOs/target-task/nightly/nightly-20261126-060807-becc1756/soak-aarch64/kt406-dns-interpose.dylib"
exec "/opt/homebrew/bin/qemu-system-aarch64" "$@"
