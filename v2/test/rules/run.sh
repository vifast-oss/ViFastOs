#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Self-test của 5 luật cấu trúc: mỗi luật phải BẮT được case âm CỦA CHÍNH NÓ
# (không chấp nhận "exit 1 vô tội vạ": đầu ra phải chứa đúng mã R<k>).
# Chạy: bash v2/test/rules/run.sh   (exit 0 = cả 6 case đúng như kỳ vọng)
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"   # thư mục v2/
V2="$V2_ROOT/bin/v2"
fail=0
total=0

# check <tên> <exit kỳ vọng> <mã luật kỳ vọng|-> <thư mục case>
check() {
    local name="$1" want="$2" rule="$3" dir="$4" out got ok=1
    total=$((total + 1))
    if [ ! -f "$V2" ]; then
        echo "not ok $total - $name (chưa có $V2)"
        fail=$((fail + 1))
        return
    fi
    out="$(cd "$V2_ROOT/.." && "$V2" rules --root "$HERE/../fixtures/$dir" 2>&1)"
    got=$?
    [ "$got" -eq "$want" ] || ok=0
    if [ "$want" -ne 0 ]; then
        printf '%s\n' "$out" | grep -q "^$rule " || ok=0
    fi
    if [ "$ok" -eq 1 ]; then
        echo "ok $total - $name"
    else
        echo "not ok $total - $name (exit $got, muốn $want; mã luật '$rule')"
        printf '%s\n' "$out" | sed 's/^/#   /'
        fail=$((fail + 1))
    fi
}

echo "TAP version 13"
echo "1..6"
check "R1 tệp > 1.500 dòng trong kernel/lib bị bắt" 1 R1 "case-r1-long"
check "R2 cfg(target_arch) trong lib bị bắt"         1 R2 "case-r2-cfg-arch"
check "R3 tệp thiếu SPDX bị bắt"                     1 R3 "case-r3-no-spdx"
check "R4 path-dependency sang crates/ bị bắt"       1 R4 "case-r4-path-dep"
check "R5 unsafe ngoài danh sách đóng bị bắt"        1 R5 "case-r5-unsafe"
check "case hợp lệ (đối chứng dương) thì PASS"       0 "-" "case-ok"

if [ "$fail" -ne 0 ]; then
    echo "# $fail/$total case sai kỳ vọng"
    exit 1
fi
echo "# $total/$total case đúng kỳ vọng"
exit 0
