#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# L25 — harness chung: biến môi trường + build_tree/run.
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=${REPO:-$(cd "$HERE/../../../.." && pwd)}
BASE=${BASE:-${TMPDIR:-/tmp}/kt187-l25}
PY=${PY:-python3}
CUR=${CUR:-$REPO/v2/tools/check-wake-paths.py}
PRE=${PRE:-$BASE/check-pre184.py}
OLD=${OLD:-$BASE/check-kt174.py}
B6=${B6:-$BASE/check-b6efc403.py}

build_tree() {
    # $1 = dest
    rm -rf "$1"
    mkdir -p "$1/v2"
    cp -R "$REPO/v2/kernel" "$1/v2/kernel"
    cp -R "$REPO/v2/sim" "$1/v2/sim"
    cp -R "$REPO/v2/lib" "$1/v2/lib"
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
