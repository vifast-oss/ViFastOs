#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-04c + KT-09: fixture âm — máy kiểm phải BẮT được **đúng thông điệp lỗi**,
# không chỉ "exit khác 0" (mọi fixture một tệp đều trượt luật "đúng 21 tệp", nên
# chỉ kiểm exit code là kiểm rỗng — KT-09 siết lại, D2-41).
set -u
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
PY="${PYTHON:-python3.12}"
CHECK="$ROOT/v2/test/scn/check_scn.py"
fails=0
n=0
run() { # <fixture> <chuỗi con bắt buộc có trong log>
    n=$((n + 1))
    local dir="$1" want="$2" log
    log="/tmp/scn-fix-$dir.log"
    if "$PY" "$CHECK" --dir "$ROOT/v2/test/scn/fixtures/$dir" >"$log" 2>&1; then
        echo "not ok $n - fixture $dir phải bị bắt"
        fails=$((fails + 1))
    elif ! grep -Fq -- "$want" "$log"; then
        echo "not ok $n - fixture $dir bị bắt nhưng KHÔNG đúng thông điệp"
        echo "#   thiếu: $want"
        fails=$((fails + 1))
    else
        echo "ok $n - fixture $dir bị bắt đúng thông điệp"
    fi
}
echo "TAP version 13"
echo "1..5"
run bad-method "CNode.Frobnicate không có trong spec/abi.toml"
run bad-error "kết quả 'NoSuchError' không phải ok/19 mã lỗi"
run dup-name "name 'dup-name' trùng"
run bad-arity "CNode.Copy thiếu tham số bắt buộc ['dst']"
run bad-state-value "giá trị 'maybe' của trường 'call' không hợp lệ"
if [ "$fails" -eq 0 ]; then echo "# scn fixtures: PASS"; exit 0; fi
echo "# scn fixtures: FAIL ($fails)"; exit 1
