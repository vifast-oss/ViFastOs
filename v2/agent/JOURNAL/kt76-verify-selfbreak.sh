#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-76 — **tự phá** bộ kiểm số (D2-112) trên **BẢN COPY** (D2-113F): cây thật **không** bị sửa.
#
# Vì sao: D2-113F rút từ KT-73 — script tự phá mà sửa cây thật thì có race với verifier đang chạy
# và có thể commit tài liệu bị phá dở. Script này:
#   1. copy các tệp **đầu vào** sang `mktemp -d` (giữ cấu trúc `<tmp>/v2/...` + `<tmp>/docs/...`);
#   2. chạy verifier trên bản copy ⇒ phải `fail=0` (đối chứng);
#   3. **năm** ca phá, mỗi ca phải **đỏ đúng lý do** (so khớp chuỗi lý do, không chỉ exit ≠ 0);
#   4. chứng minh md5 của **cây thật** (tệp đầu vào) **trước = sau**.
#
# Dùng: bash v2/agent/JOURNAL/kt76-verify-selfbreak.sh
# Exit: 0 = cả bốn ca đỏ đúng + cây thật không đổi; 1 = có ca sai.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2="$(cd "$HERE/../.." && pwd)"
REPO="$(cd "$V2/.." && pwd)"

fail=0
ok=0
report() { echo "$*"; }
expect_fail() { # <tên ca> <log> <chuỗi lý do phải thấy>
    if grep -q "$3" "$2"; then
        ok=$((ok + 1))
        report "# selfbreak: ĐỎ ĐÚNG LÝ DO — $1 [$3]"
    else
        fail=$((fail + 1))
        report "# selfbreak: SAI — $1 KHÔNG đỏ với lý do [$3]"
        sed -n '1,12p' "$2" | sed 's/^/#   /'
    fi
}

# md5 của **tệp đầu vào** (không gồm log do chính script sinh) — chứng minh cây thật không đổi.
tree_hash() {
    cat "$V2/agent/reports/K6-DOD.md" \
        "$V2/docs/k3b-core.md" \
        "$V2/agent/STATE.md" \
        "$V2/agent/BLOCKED.md" \
        "$HERE/kt76-criteria.txt" \
        "$HERE/kt76-verify-numbers.sh" \
        "$HERE/kt76_verify_numbers.py" \
        "$HERE/kt76-extract-criteria.sh" \
        "$HERE/kt76_extract_criteria.py" \
        "$HERE/kt76-journal.md" 2>/dev/null | md5 -q
}
H_BEFORE="$(tree_hash)"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/kt76-selfbreak.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/v2/agent/reports" "$TMP/v2/docs" "$TMP/v2/agent/JOURNAL" "$TMP/docs/v1" "$TMP/docs/v2"

cp "$V2/agent/reports/K6-DOD.md" "$TMP/v2/agent/reports/"
cp "$V2/docs/k3b-core.md" "$TMP/v2/docs/"
cp "$V2/agent/STATE.md" "$V2/agent/BLOCKED.md" "$TMP/v2/agent/"
cp "$HERE"/kt76-* "$HERE"/kt76_* "$TMP/v2/agent/JOURNAL/" 2>/dev/null || true
# log bằng chứng của các task trước (D2-109: log đã nằm trong repo)
for f in kt70-green-qemu-ktest-user.log kt71-green-qemu-ktest.log kt74-qemu-ktest-exec.log \
         kt75-persist-run.log kt74-pin-calc.log; do
    [ -f "$HERE/$f" ] && cp "$HERE/$f" "$TMP/v2/agent/JOURNAL/"
done
cp "$REPO/docs/v2/ViFastOs-v2-thiet-ke.md" "$TMP/docs/v2/"
cp "$REPO/docs/v1/KNOWN-ISSUES.md" "$TMP/docs/v1/"

verify_copy() { KT76_ROOT="$TMP/v2" bash "$TMP/v2/agent/JOURNAL/kt76-verify-numbers.sh" 2>&1; }

# --- 0) đối chứng: bản copy CHƯA phá phải xanh ---
OUT0="$TMP/ca0.log"
verify_copy >"$OUT0" || true
if grep -q 'fail=0' "$OUT0"; then
    ok=$((ok + 1)); report "# selfbreak: ĐỐI CHỨNG — bản copy chưa phá: fail=0"
else
    fail=$((fail + 1)); report "# selfbreak: SAI — bản copy chưa phá đã đỏ:"; sed -n '1,15p' "$OUT0"
fi

# --- 1) phá số ĐO trong khối scope: dòng không còn trong log ---
DOD_COPY="$TMP/v2/agent/reports/K6-DOD.md"
sed -i '' 's/^TESTS=729 \/ SUITES=100$/TESTS=728 \/ SUITES=100/' "$DOD_COPY"
OUT1="$TMP/ca1.log"
verify_copy >"$OUT1" || true
expect_fail "phá số đo trong khối scope" "$OUT1" "dòng không có trong log kt76-baseline-gate-summary.log"
cp "$V2/agent/reports/K6-DOD.md" "$DOD_COPY"

# --- 2) đưa số ĐO trần ra prose (ngoài khối scope) ---
printf '\nsoak 999/1000\n' >>"$DOD_COPY"
OUT2="$TMP/ca2.log"
verify_copy >"$OUT2" || true
expect_fail "số đo trần ngoài khối" "$OUT2" "số '999/1000' ngoài khối scope"
cp "$V2/agent/reports/K6-DOD.md" "$DOD_COPY"

# --- 3) sửa chữ số trong TRÍCH DẪN tiêu chí (phải hết là xâu con của tiêu chí) ---
sed -i '' 's/“test-persist 7 pha”/“test-persist 8 pha”/' "$DOD_COPY"
OUT3="$TMP/ca3.log"
verify_copy >"$OUT3" || true
expect_fail "sửa chữ số trong trích dẫn tiêu chí" "$OUT3" "số '8' ngoài khối scope"
cp "$V2/agent/reports/K6-DOD.md" "$DOD_COPY"

# --- 4) sửa tệp tiêu chí (phải khác bản sinh lại từ nguồn đọc-only) ---
sed -i '' 's/600 boot/601 boot/' "$TMP/v2/agent/JOURNAL/kt76-criteria.txt"
OUT4="$TMP/ca4.log"
verify_copy >"$OUT4" || true
expect_fail "sửa tệp tiêu chí" "$OUT4" "KHÁC bản sinh lại từ nguồn đọc-only"
cp "$HERE/kt76-criteria.txt" "$TMP/v2/agent/JOURNAL/kt76-criteria.txt"

# --- 5) giấu số đo trong **HEADING** (reviewer KT-76: verifier cũ miễn "mọi số ở heading") ---
printf '\n## 98765\n' >>"$DOD_COPY"
OUT5="$TMP/ca5.log"
verify_copy >"$OUT5" || true
expect_fail "số đo giấu trong heading" "$OUT5" "số '98765' ngoài khối scope"
cp "$V2/agent/reports/K6-DOD.md" "$DOD_COPY"

H_AFTER="$(tree_hash)"
if [ "$H_BEFORE" = "$H_AFTER" ]; then
    ok=$((ok + 1)); report "# selfbreak: cây THẬT không đổi — md5 input trước=$H_BEFORE sau=$H_AFTER"
else
    fail=$((fail + 1)); report "# selfbreak: SAI — cây thật ĐỔI: trước=$H_BEFORE sau=$H_AFTER"
fi

if [ "$fail" -eq 0 ]; then
    report "# selfbreak: PASS ($ok/$((ok + fail)) ca đúng, gồm đối chứng + năm ca phá + md5 cây thật)"
    echo "# kt76-verify-selfbreak: PASS fail=0"
    exit 0
fi
report "# selfbreak: FAIL ($fail ca sai / $((ok + fail)) ca)"
echo "# kt76-verify-selfbreak: FAIL fail=$fail"
exit 1
