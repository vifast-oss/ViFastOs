#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-189 / L28 — harness độc lập (không dùng script của tác giả hay của L25).
set -u
REPO=${REPO:-/tmp/night2-L28-review187}
BASE=${BASE:-/tmp/kt189-l28}
PY=${PY:-python3}
CUR=${CUR:-$REPO/v2/tools/check-wake-paths.py}
OLD=${OLD:-$BASE/check-8670cdfe.py}
export PATH="$HOME/.cargo/bin:$PATH"

# Cây chỉ có nguồn (đủ cho checker) — dùng cho hầu hết phép thử.
build_src() {
    rm -rf "$1"
    mkdir -p "$1/v2"
    cp -R "$REPO/v2/kernel" "$1/v2/kernel"
    cp -R "$REPO/v2/sim" "$1/v2/sim"
    cp -R "$REPO/v2/lib" "$1/v2/lib"
}

# Cây đầy đủ để `cargo check` (workspace v2 + kernel workspace).
build_full() {
    build_src "$1"
    cp "$REPO/v2/Cargo.toml" "$1/v2/"
    cp "$REPO/v2/Cargo.lock" "$1/v2/"
    cp "$REPO/v2/rust-toolchain.toml" "$1/v2/"
    mkdir -p "$1/v2/tools"
    cp -R "$REPO/v2/tools/vifast-agentctl" "$1/v2/tools/"
}

run() {
    # $1 = label, $2 = checker, $3 = root, rest = flags
    label="$1"; checker="$2"; root="$3"; shift 3
    out=$("$PY" "$checker" --root "$root" "$@" 2>&1)
    rc=$?
    echo "### $label"
    echo "$out" | grep -E '^E |^# check-wake' || true
    echo "rc=$rc"
    echo
}

runq() {
    # chỉ in dòng tổng kết + rc
    label="$1"; checker="$2"; root="$3"; shift 3
    out=$("$PY" "$checker" --root "$root" "$@" 2>&1)
    rc=$?
    echo "### $label"
    echo "$out" | grep -E '^# check-wake' || true
    echo "rc=$rc"
    echo
}
