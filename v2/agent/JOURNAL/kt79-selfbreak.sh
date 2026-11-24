#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-79 (D2-113F) — selfbreak của script kiểm số: chạy trên **BẢN COPY**, không sửa cây thật.
# Ca phá: đổi `82552` (cỡ ảnh canonical) → `99999` trong `k6-repro.md` của bản copy ⇒ verifier
# PHẢI đỏ (exit 1, in MISSING 99999). Sau đó chứng minh md5 của tài liệu trong cây thật KHÔNG đổi.
set -u
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$HERE/../../.." && pwd)"
VERIFY="$HERE/kt79-verify-numbers.sh"
REAL="$REPO_ROOT/v2/docs/k6-repro.md"
[ -f "$REAL" ] || { echo "selfbreak: thiếu $REAL" >&2; exit 2; }

BEFORE="$(md5 -q "$REAL" 2>/dev/null || md5sum "$REAL" | awk '{print $1}')"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/kt79-selfbreak.XXXXXX")"
mkdir -p "$TMP/v2/docs" "$TMP/v2/agent/JOURNAL"
cp "$REAL" "$TMP/v2/docs/k6-repro.md"
cp "$REPO_ROOT/v2/docs/k3b-core.md" "$TMP/v2/docs/k3b-core.md" 2>/dev/null || true
ln -s "$HERE" "$TMP/v2/agent/JOURNAL-link" 2>/dev/null || true

# verifier đọc log từ thư mục của CHÍNH nó; nên chạy bản copy của verifier trỏ JOURNAL thật.
cp "$VERIFY" "$TMP/v2/agent/JOURNAL/kt79-verify-numbers.sh"
cp "$HERE"/kt79-*.log "$TMP/v2/agent/JOURNAL/" 2>/dev/null || true
sed -i.bak 's/82552/99999/' "$TMP/v2/docs/k6-repro.md"
grep -q 99999 "$TMP/v2/docs/k6-repro.md" || { echo "selfbreak: không sửa được bản copy" >&2; exit 2; }

OUT="$(bash "$TMP/v2/agent/JOURNAL/kt79-verify-numbers.sh" --docs-root "$TMP" 2>&1)"
RC=$?
echo "$OUT" | tail -4
AFTER="$(md5 -q "$REAL" 2>/dev/null || md5sum "$REAL" | awk '{print $1}')"
echo "# selfbreak: verifier_exit=$RC (mong đợi 1) | md5_trước=$BEFORE | md5_sau=$AFTER"
if [ "$RC" -eq 1 ] && echo "$OUT" | grep -q "MISSING 99999" && [ "$BEFORE" = "$AFTER" ]; then
    echo "ok - selfbreak ĐỎ đúng, cây thật không đổi"
    rm -rf "$TMP"
    exit 0
fi
echo "not ok - selfbreak không đỏ hoặc cây thật bị đổi"
rm -rf "$TMP"
exit 1
