#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-74 — **tự phá** script kiểm số (D2-110A) trên **BẢN COPY**, theo luật **F của D2-113**:
# script tự phá **KHÔNG** được sửa cây thật (KT-73 đã gây race: verifier chạy đúng lúc script đang
# khôi phục ⇒ thấy số giả và suýt commit tài liệu bị phá dở). Vì vậy:
#
#   1. `cp -R` **đúng** ba tệp tài liệu + log của KT-74 sang `$TMP/kt74-break`;
#   2. chạy verifier với `KT74_DOCS_ROOT`/`KT74_LOG_ROOT` trỏ vào **bản copy** ⇒ phải PASS;
#   3. phá **một** con số trong bản copy (đổi một chữ số của một dòng evidence) ⇒ verifier phải
#      FAIL (`fail>0`) — chứng minh nó thật sự kiểm, không phải luôn xanh;
#   4. khôi phục bản copy ⇒ PASS lại;
#   5. chứng minh **md5 cây thật không đổi** trước/sau toàn bộ quá trình (so md5 của ba tệp).
#
# Dùng: bash v2/agent/JOURNAL/kt74-verify-selfbreak.sh
# Exit: 0 = (PASS → FAIL → PASS) + md5 cây thật không đổi; 1 = script kiểm **không** bắt được đột biến.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
TMP="${KT74_BREAK_DIR:-/tmp/kt74-break}"
VERIFY="$HERE/kt74-verify-numbers.sh"
DOCS=("docs/k6-exec.md" "docs/k3b-core.md" "agent/STATE.md")

rm -rf "$TMP"
mkdir -p "$TMP/docs" "$TMP/agent"
for f in "${DOCS[@]}"; do
    mkdir -p "$TMP/$(dirname "$f")"
    cp "$V2_ROOT/$f" "$TMP/$f" || {
        echo "selfbreak: không copy được $f" >&2
        exit 2
    }
done
# Log nguồn của KT-74 (chỉ `.log`) + **bài journal** (để bản copy kiểm đủ 4 tài liệu như cây thật).
cp "$HERE"/kt74-*.log "$TMP/agent/" 2>/dev/null || true
mkdir -p "$TMP/agent/JOURNAL"
cp "$HERE"/kt74-journal.md "$TMP/agent/JOURNAL/" 2>/dev/null || true

md5_of_real() {
    for f in "${DOCS[@]}"; do
        md5 -q "$V2_ROOT/$f" 2>/dev/null || md5sum "$V2_ROOT/$f" | cut -d' ' -f1
    done | tr '\n' ' '
}

BEFORE="$(md5_of_real)"

# `run_verify <docs-root> <log-root> <out-file>`: trả **exit của verifier** (không phải của `tail`)
# — bài học D2-113: đọc `$?` của `tail` luôn bằng 0 nên "FAIL mong đợi" không bao giờ thấy.
run_verify() {
    KT74_DOCS_ROOT="$1" KT74_LOG_ROOT="$2" bash "$VERIFY" >"$3" 2>&1
    local rc=$?
    tail -20 "$3"
    return "$rc"
}

echo "# selfbreak 1/4: bản copy **chưa** phá ⇒ phải PASS"
if ! run_verify "$TMP" "$TMP/agent" "$TMP/step1.out"; then
    cat "$TMP/step1.out"
    echo "selfbreak: bản copy nguyên vẹn mà FAIL — dừng" >&2
    exit 2
fi
grep -E "^# kt74-verify-numbers: .*fail=0$" "$TMP/step1.out" >/dev/null || {
    cat "$TMP/step1.out"
    echo "selfbreak: không thấy dòng tổng kết fail=0" >&2
    exit 2
}

echo "# selfbreak 2/4: phá **một** chữ số trong bản copy ⇒ phải FAIL"
cp "$TMP/docs/k6-exec.md" "$TMP/k6-exec.md.good"
# Đột biến **có chủ đích**: đổi `frames=11` thành `frames=99` trong dòng evidence của chuỗi.
python3.12 - "$TMP/docs/k6-exec.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
needle = "recycled=33 freed=33"
assert needle in text, "không thấy dòng evidence để phá"
open(path, "w", encoding="utf-8").write(text.replace(needle, "recycled=99 freed=33", 1))
print("đã phá: recycled=33 -> recycled=99")
PY
if run_verify "$TMP" "$TMP/agent" "$TMP/step2.out"; then
    cat "$TMP/step2.out"
    echo "selfbreak: verifier **KHÔNG** bắt được đột biến (FAIL mong đợi) — script kiểm vô dụng" >&2
    cp "$TMP/k6-exec.md.good" "$TMP/docs/k6-exec.md"
    exit 1
fi
grep -E "^FAIL docs/k6-exec.md: dòng không có trong kt74-qemu-ktest-exec.log" "$TMP/step2.out" >/dev/null || {
    cat "$TMP/step2.out"
    echo "selfbreak: FAIL nhưng không phải vì dòng evidence bị phá — kiểm lại" >&2
    cp "$TMP/k6-exec.md.good" "$TMP/docs/k6-exec.md"
    exit 1
}

echo "# selfbreak 3/4: khôi phục bản copy ⇒ phải PASS lại"
cp "$TMP/k6-exec.md.good" "$TMP/docs/k6-exec.md"
if ! run_verify "$TMP" "$TMP/agent" "$TMP/step3.out"; then
    cat "$TMP/step3.out"
    echo "selfbreak: khôi phục xong mà vẫn FAIL" >&2
    exit 1
fi

echo "# selfbreak 4/4: chèn **số trần ở prose** vào bản copy ⇒ phải FAIL (D1 reviewer)"
#
# Trước bản vá D1, verifier đặt `heading = True` ở tiêu đề đầu rồi **không reset** ⇒ mọi số nguyên
# trần trong prose đều được miễn; đột biến kiểu này **lọt**. Bước này khoá lại đúng lỗ đó.
cp "$TMP/docs/k6-exec.md" "$TMP/k6-exec.md.good2"
python3.12 - "$TMP/docs/k6-exec.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
needle = "**tăng gấp đôi**"
assert needle in text, "không thấy cụm prose để phá"
open(path, "w", encoding="utf-8").write(text.replace(needle, "**22**", 1))
print("đã phá prose: '**tăng gấp đôi**' -> '**22**'")
PY
if run_verify "$TMP" "$TMP/agent" "$TMP/step4.out"; then
    cat "$TMP/step4.out"
    echo "selfbreak: verifier **KHÔNG** bắt được số trần ở prose (D1 chưa kín)" >&2
    cp "$TMP/k6-exec.md.good2" "$TMP/docs/k6-exec.md"
    exit 1
fi
grep -E "^FAIL docs/k6-exec.md: số '22' ngoài khối scope" "$TMP/step4.out" >/dev/null || {
    cat "$TMP/step4.out"
    echo "selfbreak: FAIL nhưng không phải vì số trần ở prose — kiểm lại" >&2
    cp "$TMP/k6-exec.md.good2" "$TMP/docs/k6-exec.md"
    exit 1
}
cp "$TMP/k6-exec.md.good2" "$TMP/docs/k6-exec.md"
echo "# selfbreak 4/4: khôi phục bản copy ⇒ PASS lại"
if ! run_verify "$TMP" "$TMP/agent" "$TMP/step5.out"; then
    cat "$TMP/step5.out"
    echo "selfbreak: khôi phục sau đột biến prose mà vẫn FAIL" >&2
    exit 1
fi

AFTER="$(md5_of_real)"
if [ "$BEFORE" != "$AFTER" ]; then
    echo "selfbreak: md5 cây THẬT đã đổi — luật D2-113F bị vi phạm" >&2
    echo "  before=$BEFORE" >&2
    echo "  after =$AFTER" >&2
    exit 1
fi

echo "# selfbreak: PASS -> FAIL(evidence) -> PASS -> FAIL(số ở prose) -> PASS; md5 cây thật KHÔNG đổi (${BEFORE})"
