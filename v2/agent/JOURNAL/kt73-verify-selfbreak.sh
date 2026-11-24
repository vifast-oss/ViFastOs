#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-73 — **tự phá** script kiểm số (D2-112) theo **năm lớp**.
#
# **Không bao giờ ghi vào cây thật**: copy tài liệu + log sang thư mục tạm, phá **ở đó**, chạy
# verifier với `KT73_DOCS_ROOT=<bản copy>`, khôi phục **bản copy**, rồi xoá. Cây đang review không
# bị chạm (bài học: bản trước sửa tại chỗ ⇒ commit có thể chứa tài liệu bị phá dở).
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
J="$ROOT/v2/agent/JOURNAL"
VERIFY="$J/kt73-verify-numbers.sh"
LOG="$J/kt73-verify-selfbreak.log"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/kt73-sbroot.XXXXXX")" || exit 2
trap 'rm -rf "$WORK"' EXIT

mkdir -p "$WORK/v2/docs" "$WORK/v2/agent/JOURNAL"
cp "$ROOT/v2/docs/k6-console.md" "$ROOT/v2/docs/k3b-core.md" "$WORK/v2/docs/"
cp "$ROOT/v2/agent/STATE.md" "$WORK/v2/agent/"
cp "$J"/kt73-*.log "$J"/kt73-qemu-*.log "$J"/kt73-con-chardev.out "$J"/kt72-product-sha.log "$J"/kt72-soak.log "$WORK/v2/agent/JOURNAL/" 2>/dev/null
cp "$J"/kt73-journal.md "$WORK/v2/agent/JOURNAL/"

pass=0
fail=0
tmpfile="$(mktemp "${TMPDIR:-/tmp}/kt73-sbcase.XXXXXX")"

check() { # $1 tên ca, $2 tệp (trong bản copy), $3 biểu thức sed, $4 mô tả
    local name="$1" file="$2" expr="$3" what="$4"
    cp "$file" "$tmpfile.bak"
    sed -i '' -e "$expr" "$file" 2>/dev/null
    # BSD sed trả 0 **kể cả khi không khớp** ⇒ so nội dung để biết ca có áp dụng được không.
    if cmp -s "$tmpfile.bak" "$file"; then
        echo "not ok - $name: ca KHÔNG áp dụng được (mẫu không khớp)"
        fail=$((fail + 1))
        cp "$tmpfile.bak" "$file"
        return
    fi
    if KT73_DOCS_ROOT="$WORK" bash "$VERIFY" >"$tmpfile.out" 2>&1; then
        echo "not ok - $name: verifier vẫn PASS sau khi $what"
        fail=$((fail + 1))
    else
        echo "ok - $name: verifier FAIL sau khi $what"
        grep -E "^not ok" "$tmpfile.out" | head -3 | sed 's/^/    /'
        pass=$((pass + 1))
    fi
    cp "$tmpfile.bak" "$file"
    if ! KT73_DOCS_ROOT="$WORK" bash "$VERIFY" >"$tmpfile.out2" 2>&1; then
        echo "not ok - $name: verifier FAIL sau khi khôi phục bản copy"
        fail=$((fail + 1))
    fi
}

{
    echo "# KT-73 tự phá (D2-112) — $(date -u +%Y-%m-%dT%H:%M:%SZ) — chạy trên BẢN COPY ($WORK)"
    D="$WORK/v2/docs/k6-console.md"
    ST="$WORK/v2/agent/STATE.md"
    check "hoán-số-giữa-ảnh" "$D" 's/data\+bss=206736/data+bss=202832/' "hoán số ảnh ktest-proc thành số của ảnh ktest-user"
    check "số-đo-một-chữ-số" "$D" 's/drops=2/drops=5/' "đổi drops=2 thành drops=5"
    check "số-ngoài-khối" "$D" 's/^## 8\. Cổng (log trong repo)/## 8. Cổng (log trong repo) — tổng 214152/' "chèn số ngoài khối"
    check "log-mutation-làm-nguồn" "$D" 's/scope=kt73-qemu-ktest-con\.log/scope=kt73-mutation-tap3.log/' "đổi phạm vi sang log mutation"
    check "STATE-ngoài-khối" "$ST" 's/console \*\*poll\*\* chưa dùng ngắt/console **poll** chưa dùng ngắt (0 ca)/' "thêm số vào STATE ngoài khối"
    echo "# kt73-selfbreak: PASS=$pass FAIL=$fail"
    [ "$fail" -eq 0 ] && echo "# kt73-selfbreak: PASS" || echo "# kt73-selfbreak: FAIL"
} >"$LOG" 2>&1
rc=0
grep -q "^# kt73-selfbreak: PASS$" "$LOG" || rc=1
tail -3 "$LOG"
rm -f "$tmpfile" "$tmpfile.bak" "$tmpfile.out" "$tmpfile.out2"
exit "$rc"
