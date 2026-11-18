#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-80 / OD-K6-5 luật 3 — in dòng evidence `counter=<tên>` bằng cách **đọc giá trị thật** từ
# log QEMU của KT-78 (không viết tay): bộ đếm phải có cả đường xanh (=0) và ca âm (≠0).
# Fail-closed: thiếu log/không đọc được giá trị ⇒ exit 1, không in gì.
#
# Dùng: bash v2/test/rules/counter-evidence.sh [--root R] >v2/agent/JOURNAL/kt80-*.log
set -u
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
if [ "${1:-}" = "--root" ] && [ -n "${2:-}" ]; then
    ROOT="$2"
fi
J="$ROOT/v2/agent/JOURNAL"
POS_LOG="$J/kt78-ktest-matrix.log"
NEG_LOG="$J/kt78-guard/selftest-20261117T015708Z.log"


[ -f "$POS_LOG" ] || { echo "counter-evidence: thiếu $POS_LOG" >&2; exit 1; }
[ -f "$NEG_LOG" ] || { echo "counter-evidence: thiếu $NEG_LOG" >&2; exit 1; }

value() { # <tệp> <tên bộ đếm> <0|khac0>
    local file="$1" name="$2" want="$3" vals
    vals="$(grep -o "${name}=[0-9][0-9]*" "$file" 2>/dev/null | cut -d= -f2 | sort -u)"
    if [ "$want" = "0" ]; then
        printf '%s\n' "$vals" | grep -qx '0' || return 1
        echo 0
    else
        printf '%s\n' "$vals" | grep -vx '0' | head -1
    fi
}

emit() { # <tên> <tệp xanh> <tệp âm>
    local name="$1" pos neg
    pos="$(value "$POS_LOG" "$name" 0)" || { echo "counter-evidence: $name không có giá trị 0 trong $POS_LOG" >&2; exit 1; }
    neg="$(value "$NEG_LOG" "$name" khac0)" || true
    [ -n "$neg" ] || { echo "counter-evidence: $name không có ca âm ≠0 trong $NEG_LOG" >&2; exit 1; }
    echo "counter=$name positive=$pos negative=$neg positive_log=kt78-ktest-matrix.log negative_log=kt78-guard/selftest-20261117T015708Z.log"
}

# **KT-134 (Agent 0, phần 3 — rà selftest mồ côi)**: trước bản này script **không** có chế độ tự
# kiểm, nhưng `v2/bin/v2` lại gọi `counter-evidence.sh --selftest` và cờ bị **bỏ qua im lặng** ⇒
# bước cổng mang nhãn "selftest" mà thực chất chỉ chạy lại phép kiểm thường. Đúng lớp "xanh giả".
# Nay có `--selftest` thật: dựng log giả trong thư mục tạm và phá từng bất biến.
if [ "${1:-}" = "--selftest" ]; then
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/kt80-ce.XXXXXX")" || exit 1
    trap 'rm -rf "$tmp"' EXIT
    ok=0
    bad=0
    run_case() { # <tên> <kỳ vọng: pass|fail> <nội dung pos> <nội dung neg|-> <có tệp neg?>
        local name="$1" want="$2" posbody="$3" negbody="$4" hasneg="$5" got=""
        printf '%s\n' "$posbody" >"$tmp/pos.log"
        if [ "$hasneg" = "1" ]; then printf '%s\n' "$negbody" >"$tmp/neg.log"; else rm -f "$tmp/neg.log"; fi
        POS_LOG="$tmp/pos.log"
        NEG_LOG="$tmp/neg.log"
        if out="$(emit qemu_ppid1 2>&1)"; then got="pass"; else got="fail"; fi
        if [ "$got" = "$want" ]; then
            echo "ok - selftest $name: $want đúng"
            ok=$((ok + 1))
        else
            echo "not ok - selftest $name: mong $want, thấy $got — [$out]" >&2
            bad=$((bad + 1))
        fi
    }
    run_case "dương: có 0 ở log xanh và khác 0 ở log âm" pass \
        "# x qemu_ppid1=0" "# x qemu_ppid1=3" 1
    run_case "âm: log xanh không có giá trị 0" fail \
        "# x qemu_ppid1=2" "# x qemu_ppid1=3" 1
    run_case "âm: log âm chỉ có 0 (không có ca âm thật)" fail \
        "# x qemu_ppid1=0" "# x qemu_ppid1=0" 1
    run_case "âm: thiếu log âm" fail \
        "# x qemu_ppid1=0" "" 0
    echo "# counter-evidence-selftest: $([ "$bad" -eq 0 ] && echo PASS || echo FAIL) ($ok/$((ok + bad)) ca)"
    [ "$bad" -eq 0 ] || exit 1
    exit 0
fi

emit qemu_ppid1
emit isolation_bad
echo "# counter-evidence: PASS (giá trị đọc từ log KT-78; registry v2/test/rules/counters.toml)"
