#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-75 — **tự phá** script kiểm số (D2-110A) trên **BẢN COPY**, theo luật **F của D2-113**:
# script tự phá **KHÔNG** được sửa cây thật. Vì vậy:
#
#   1. `cp` **đúng** các tệp tài liệu + log của KT-75 sang `$TMP/kt75-break`;
#   2. chạy verifier với `KT75_DOCS_ROOT`/`KT75_LOG_ROOT` trỏ vào **bản copy** ⇒ phải PASS;
#   3. phá **một** con số trong **khối evidence** của bản copy ⇒ verifier phải FAIL;
#   4. phá **một số trần ở prose** (`22`) ⇒ verifier phải FAIL;
#   5. phá **số kiểu phân cách nghìn** ở prose (`262.144`) ⇒ verifier phải FAIL — đây là đường
#      reviewer đã lọt trước khi siết `^\d+\.\d+$` (D2-112b); giữ ca này để không tái phát;
#   6. khôi phục ⇒ PASS lại;
#   7. chứng minh **md5 cây thật không đổi** trước/sau toàn bộ quá trình.
#
# Dùng: bash v2/agent/JOURNAL/kt75-verify-selfbreak.sh
# Exit: 0 = (PASS → FAIL(evidence) → FAIL(số ở prose) → FAIL(`262.144`) → PASS) + md5 cây thật
#          không đổi; 1 = script kiểm **không** bắt được đột biến.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
TMP="${KT75_BREAK_DIR:-/tmp/kt75-break}"
VERIFY="$HERE/kt75-verify-numbers.sh"
DOCS=("docs/k6-persist.md" "docs/k3b-core.md" "agent/STATE.md" "agent/JOURNAL/kt75-journal.md")

PY=""
for c in python3.12 python3.11 python3; do
    if command -v "$c" >/dev/null 2>&1; then
        PY="$c"
        break
    fi
done
[ -n "$PY" ] || {
    echo "selfbreak: không có python3" >&2
    exit 2
}

rm -rf "$TMP"
mkdir -p "$TMP/docs" "$TMP/agent/JOURNAL"
for f in "${DOCS[@]}"; do
    mkdir -p "$TMP/$(dirname "$f")"
    cp "$V2_ROOT/$f" "$TMP/$f" || {
        echo "selfbreak: không copy được $f" >&2
        exit 2
    }
done
# Log nguồn của KT-75 (chỉ `.log`) — bản copy phải kiểm đủ như cây thật.
cp "$HERE"/kt75-*.log "$TMP/agent/JOURNAL/" 2>/dev/null || true

md5_of_real() {
    for f in "${DOCS[@]}"; do
        md5 -q "$V2_ROOT/$f" 2>/dev/null || md5sum "$V2_ROOT/$f" | cut -d' ' -f1
    done | tr '\n' ' '
}

BEFORE="$(md5_of_real)"

# `run_verify <docs-root> <log-root> <out-file>`: trả **exit của verifier** (không phải của `tail`).
run_verify() {
    KT75_DOCS_ROOT="$1" KT75_LOG_ROOT="$2" bash "$VERIFY" >"$3" 2>&1
    local rc=$?
    tail -12 "$3"
    return "$rc"
}

echo "# selfbreak 1/6: bản copy **chưa** phá ⇒ phải PASS"
if ! run_verify "$TMP" "$TMP/agent/JOURNAL" "$TMP/step1.out"; then
    cat "$TMP/step1.out"
    echo "selfbreak: bản copy nguyên vẹn mà FAIL — dừng" >&2
    exit 2
fi
grep -E "^# kt75-verify-numbers: .*fail=0$" "$TMP/step1.out" >/dev/null || {
    cat "$TMP/step1.out"
    echo "selfbreak: không thấy dòng tổng kết fail=0" >&2
    exit 2
}

echo "# selfbreak 2/6: phá **một** số trong khối evidence của bản copy ⇒ phải FAIL"
cp "$TMP/agent/STATE.md" "$TMP/STATE.md.good"
"$PY" - "$TMP/agent/STATE.md" <<'PYEOF'
import re
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
# Phá **số trong dòng ngân sách** của khối evidence (không phụ thuộc giá trị cụ thể).
pattern = re.compile(r"(# image=ktest-persist [^\n]*?file=)(\d+)")
match = pattern.search(text)
assert match, "không thấy dòng evidence ngân sách để phá"
text = text[: match.start(2)] + "999999" + text[match.end(2) :]
open(path, "w", encoding="utf-8").write(text)
print("đã phá evidence: file=<số đo> -> file=999999 trong dòng ngân sách")
PYEOF
if run_verify "$TMP" "$TMP/agent/JOURNAL" "$TMP/step2.out"; then
    cat "$TMP/step2.out"
    echo "selfbreak: verifier **KHÔNG** bắt được đột biến evidence (FAIL mong đợi)" >&2
    cp "$TMP/STATE.md.good" "$TMP/agent/STATE.md"
    exit 1
fi
grep -E "FAIL .*dòng không có trong log kt75-budget.log" "$TMP/step2.out" >/dev/null || {
    cat "$TMP/step2.out"
    echo "selfbreak: FAIL nhưng không phải vì dòng evidence bị phá — kiểm lại" >&2
    cp "$TMP/STATE.md.good" "$TMP/agent/STATE.md"
    exit 1
}
cp "$TMP/STATE.md.good" "$TMP/agent/STATE.md"

echo "# selfbreak 3/6: chèn **số trần ở prose** vào bản copy ⇒ phải FAIL (D2-112b)"
cp "$TMP/docs/k6-persist.md" "$TMP/k6-persist.md.good"
"$PY" - "$TMP/docs/k6-persist.md" <<'PYEOF'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
needle = "Đây là **đối chứng**:"
assert needle in text, "không thấy cụm prose để phá"
open(path, "w", encoding="utf-8").write(text.replace(needle, "Đây là **22** **đối chứng**:", 1))
print("đã phá prose: chèn số trần '22'")
PYEOF
if run_verify "$TMP" "$TMP/agent/JOURNAL" "$TMP/step3.out"; then
    cat "$TMP/step3.out"
    echo "selfbreak: verifier **KHÔNG** bắt được số trần ở prose — D2-112b chưa kín" >&2
    cp "$TMP/k6-persist.md.good" "$TMP/docs/k6-persist.md"
    exit 1
fi
grep -E "^FAIL docs/k6-persist.md: số '22' ngoài khối scope" "$TMP/step3.out" >/dev/null || {
    cat "$TMP/step3.out"
    echo "selfbreak: FAIL nhưng không phải vì số trần ở prose — kiểm lại" >&2
    cp "$TMP/k6-persist.md.good" "$TMP/docs/k6-persist.md"
    exit 1
}
cp "$TMP/k6-persist.md.good" "$TMP/docs/k6-persist.md"

echo "# selfbreak 4/6: chèn **số kiểu phân cách nghìn** (\`262.144\`) ở prose ⇒ phải FAIL (đường reviewer đã lọt)"
"$PY" - "$TMP/docs/k6-persist.md" <<'PYEOF'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
needle = "Đây là **đối chứng**:"
assert needle in text, "không thấy cụm prose để phá"
open(path, "w", encoding="utf-8").write(text.replace(needle, "Trần cũ 262.144 và **đối chứng**:", 1))
print("đã phá prose: chèn '262.144' (số kiểu phân cách nghìn)")
PYEOF
if run_verify "$TMP" "$TMP/agent/JOURNAL" "$TMP/step4.out"; then
    cat "$TMP/step4.out"
    echo "selfbreak: verifier **KHÔNG** bắt được '262.144' ở prose — miễn trừ '\d+\.\d+' chưa bị siết" >&2
    cp "$TMP/k6-persist.md.good" "$TMP/docs/k6-persist.md"
    exit 1
fi
grep -E "^FAIL docs/k6-persist.md: số '262\.144' ngoài khối scope" "$TMP/step4.out" >/dev/null || {
    cat "$TMP/step4.out"
    echo "selfbreak: FAIL nhưng không phải vì '262.144' — kiểm lại" >&2
    cp "$TMP/k6-persist.md.good" "$TMP/docs/k6-persist.md"
    exit 1
}
cp "$TMP/k6-persist.md.good" "$TMP/docs/k6-persist.md"

echo "# selfbreak 5/6: khôi phục bản copy ⇒ phải PASS lại"
if ! run_verify "$TMP" "$TMP/agent/JOURNAL" "$TMP/step5.out"; then
    cat "$TMP/step5.out"
    echo "selfbreak: khôi phục xong mà vẫn FAIL" >&2
    exit 1
fi

AFTER="$(md5_of_real)"
if [ "$BEFORE" != "$AFTER" ]; then
    echo "selfbreak: md5 cây THẬT đã đổi — luật D2-113F bị vi phạm" >&2
    echo "  before=$BEFORE" >&2
    echo "  after =$AFTER" >&2
    exit 1
fi

echo "# selfbreak 6/6: PASS -> FAIL(evidence) -> FAIL(số ở prose) -> FAIL(262.144) -> PASS; md5 cây thật KHÔNG đổi (${BEFORE})"
