#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-80 — **ca phá cho TỪNG luật** (OD-K6-5 luật 1, 2, 3, 7, 8), chạy trên **BẢN COPY**
# (D2-113F: self-break không bao giờ sửa cây thật; D2-107/D2-113).
#
# Với mỗi luật, script làm đúng ba bước trên một bản copy trong `mktemp -d`:
#   1. **đối chứng dương**: bản copy chưa phá ⇒ luật phải XANH;
#   2. **phá**: sửa bản copy đúng đường mà luật phải bắt;
#   3. **đối chứng âm**: luật phải ĐỎ và đúng **mã lỗi** kỳ vọng.
# Cuối cùng: `shasum` cây thật trước/sau phải **giống nhau** (0 tệp bị sửa).
#
# Chạy: bash v2/test/rules/selfbreak.sh          # exit 0 = 5/5 luật ĐỎ được + cây thật không đổi
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"   # gốc repo
RULES="$ROOT/v2/test/rules"

PY=""
for p in python3.12 python3.11 python3; do
    if command -v "$p" >/dev/null 2>&1 && "$p" -c 'import tomllib' >/dev/null 2>&1; then
        PY="$(command -v "$p")"
        break
    fi
done
[ -n "$PY" ] || {
    echo "selfbreak: cần python ≥ 3.11 có tomllib" >&2
    exit 2
}

SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/kt80-selfbreak.XXXXXX")" || exit 2
trap 'rm -rf "$SCRATCH"' EXIT

# shasum **các tệp thuộc KT-80** (bộ kiểm + registry + tài liệu luật): đây là "cây thật" mà
# self-break có thể làm hỏng. KHÔNG băm cả `v2/docs`/`v2/agent/reports`: các task song song đang
# sửa tệp của họ (D2-105) ⇒ băm toàn thư mục sẽ báo động giả giữa lúc chạy. KHÔNG băm
# `JOURNAL/kt80-*`: đó là **đầu ra** của chính lần chạy (log do `tee` ghi). Tệp của task khác là
# **đầu vào** (copy từ HEAD cho tệp ghim sha), không phải đầu ra của script này.
hash_real_tree() {
    (
        cd "$ROOT" || exit 1
        {
            printf '%s\0' v2/bin/v2 v2/tools/verify-numbers.py v2/docs/k6-machine-rules.md
            # **KT-333d (D2-245)** — `v2/bin/v2` đã tách thành dispatcher + `v2/bin/lib/*.sh`;
            # băm **cả** các mảnh đó, nếu không phép thử "self-break không sửa cây thật" sẽ bỏ
            # sót đúng nơi thân lệnh đang nằm.
            find v2/bin/lib -type f -print0 2>/dev/null
            find v2/test/rules -type f -print0
        } | sort -z | xargs -0 shasum -a 256 | shasum -a 256 | awk '{print $1}'
    )
}

BEFORE="$(hash_real_tree)"
total=0
fail=0
declare -a NAMES=()
declare -a RESULTS=()

# copy_repo <đích>: bản copy tối thiểu nhưng **đủ** cho cả 5 luật (docs, reports, JOURNAL, tools,
# test/rules + hai nguồn đọc-only của luật 7).
copy_repo() {
    local dst="$1"
    mkdir -p "$dst"
    # Chỉ chép tệp **đã có trong index** — đúng tập tài liệu đích mà verifier kiểm, và không
    # chép bản nháp chưa `git add` của task song song (D2-105a).
    "$PY" - "$ROOT" "$dst" <<'PYEOF'
import os
import shutil
import subprocess
import sys

root, dst = sys.argv[1], sys.argv[2]
paths = subprocess.run(
    ["git", "-C", root, "ls-files", "-z", "v2/agent", "v2/docs", "v2/test", "v2/tools"],
    capture_output=True, check=True,
).stdout.split(b"\0")
paths += [b"docs/v1/KNOWN-ISSUES.md", b"docs/v2/ViFastOs-v2-thiet-ke.md"]
# Tệp ghim sha trong [[legacy]] lấy **bản HEAD** (đã duyệt). Tệp đang được task khác sửa trong
# working tree là bản nháp — không chép, để phép thử không phụ thuộc bản nháp của task khác
# (D2-105b: xác minh bằng HEAD). Mục [[excluded]] không phải tài liệu đích: bỏ.
import tomllib
policy = tomllib.load(open(os.path.join(root, "v2/test/rules/numbers-policy.toml"), "rb"))
pinned = {e["path"] for e in policy.get("legacy", [])}
excluded = {e["path"] for e in policy.get("excluded", [])}
for raw in paths:
    rel = raw.decode()
    if not rel or rel in excluded:
        continue
    target = os.path.join(dst, rel)
    if rel in pinned:
        blob = subprocess.run(["git", "-C", root, "show", f"HEAD:{rel}"],
                              capture_output=True, check=True).stdout
        os.makedirs(os.path.dirname(target), exist_ok=True)
        open(target, "wb").write(blob)
        continue
    if not os.path.isfile(os.path.join(root, rel)):
        continue
    os.makedirs(os.path.dirname(target), exist_ok=True)
    shutil.copy2(os.path.join(root, rel), target)
PYEOF
}

git_init() {
    git -C "$1" init -q
    git -C "$1" config user.email "kt80@example.invalid"
    git -C "$1" config user.name "kt80"
}

# run_law <root> <luật> [mã kỳ vọng] -> in "EXIT mã" và trả 0 nếu khớp kỳ vọng
#   mã rỗng = kỳ vọng XANH (exit 0); mã khác rỗng = kỳ vọng ĐỎ + chứa mã.
run_law() {
    local copy="$1" law="$2" want="${3:-}"
    local out rc
    out="$("$PY" "$copy/v2/tools/verify-numbers.py" --root "$copy" --law "$law" 2>&1)"
    rc=$?
    LAST_OUT="$out"
    if [ -z "$want" ]; then
        [ "$rc" -eq 0 ] && return 0
        echo "    (kỳ vọng XANH nhưng exit=$rc)" >&2
        printf '%s\n' "$out" | sed 's/^/    /' >&2
        return 1
    fi
    if [ "$rc" -eq 0 ]; then
        echo "    (kỳ vọng ĐỎ [$want] nhưng luật XANH)" >&2
        return 1
    fi
    if ! printf '%s\n' "$out" | grep -q -- "$want"; then
        echo "    (ĐỎ nhưng KHÔNG có mã [$want])" >&2
        printf '%s\n' "$out" | sed 's/^/    /' >&2
        return 1
    fi
    return 0
}

check_case() { # <tên> <hàm chạy>
    local name="$1" fn="$2" dir
    total=$((total + 1))
    dir="$SCRATCH/case-$total"
    NAMES+=("$name")
    if "$fn" "$dir"; then
        RESULTS+=("ok")
        echo "ok $total - $name"
    else
        RESULTS+=("not ok")
        fail=$((fail + 1))
        echo "not ok $total - $name"
    fi
}

# ------------------------------------------------------------------ luật 1
case_law1() {
    local d="$1"
    copy_repo "$d"
    run_law "$d" 1 || return 1
    echo "    đối chứng dương: luật 1 XANH trên bản copy"
    # phá: số đo dạng `key=value` **ngoài** khối scope trên tài liệu đang bị kiểm
    # (đúng đường phá của reviewer KT-80: `drops=5` ở prose từng lọt — D2-109C + phản biện)
    printf '\nSố test của cổng: drops=5.\n' >>"$d/v2/docs/k6-machine-rules.md"
    run_law "$d" 1 E1_NUMBER_ASSIGNED
}

# ------------------------------------------------------------------ luật 2
case_law2() {
    local d="$1"
    # (a) đối chứng dương: log commit TRƯỚC báo cáo.
    copy_repo "$d"
    git_init "$d"
    git -C "$d" add v2/agent/JOURNAL >/dev/null 2>&1
    git -C "$d" commit -qm "log trước" >/dev/null 2>&1
    git -C "$d" add -A >/dev/null 2>&1
    git -C "$d" commit -qm "báo cáo sau" >/dev/null 2>&1
    run_law "$d" 2 || return 1
    echo "    đối chứng dương: luật 2 XANH khi log commit trước"
    # (b) ca phá: báo cáo commit TRƯỚC, log commit SAU (cùng nội dung tệp trên đĩa).
    local w="$d-wrong"
    copy_repo "$w"
    git_init "$w"
    git -C "$w" add v2/agent/reports v2/docs >/dev/null 2>&1
    git -C "$w" commit -qm "báo cáo trước" >/dev/null 2>&1
    git -C "$w" add -A >/dev/null 2>&1
    git -C "$w" commit -qm "log sau" >/dev/null 2>&1
    run_law "$w" 2 E2_ORDER
}

# ------------------------------------------------------------------ luật 3
case_law3() {
    local d="$1"
    copy_repo "$d"
    run_law "$d" 3 || return 1
    echo "    đối chứng dương: luật 3 XANH trên bản copy"
    # phá: bộ đếm đang được trích trong khối ok bị hạ xuống proven=false (D2-108B)
    "$PY" - "$d" <<'PYEOF'
import sys
p = sys.argv[1] + "/v2/test/rules/counters.toml"
out = []
done = False
for line in open(p, encoding="utf-8").read().splitlines():
    if not done and line.strip().startswith("proven = true"):
        line = line.replace("proven = true", "proven = false")
        done = True
    out.append(line)
assert done, "không thấy mục proven = true"
open(p, "w", encoding="utf-8").write("\n".join(out) + "\n")
PYEOF
    run_law "$d" 3 E3_UNPROVEN_IN_OK
}

# ------------------------------------------------------------------ luật 7
case_law7() {
    local d="$1"
    copy_repo "$d"
    run_law "$d" 7 || return 1
    echo "    đối chứng dương: luật 7 XANH trên bản copy"
    # phá: bỏ một cột của dòng N3 trong bản trích tiêu chí (D2-118B)
    "$PY" - "$d" <<'PYEOF'
import sys
p = sys.argv[1] + "/v2/agent/JOURNAL/kt76-criteria.txt"
lines = open(p, encoding="utf-8").read().splitlines()
out = []
for line in lines:
    if line.startswith("N3 |"):
        cells = line.split("|")
        line = "|".join(cells[:-2] + cells[-1:])
    out.append(line)
open(p, "w", encoding="utf-8").write("\n".join(out) + "\n")
PYEOF
    run_law "$d" 7 E7_COLUMN_MISMATCH
}

# ------------------------------------------------------------------ luật 8
case_law8() {
    local d="$1"
    mkdir -p "$d/v2/test/rules" "$d/v2/tools" "$d/v2/agent/JOURNAL" "$d/v2/agent/reports"
    cp "$ROOT/v2/tools/verify-numbers.py" "$d/v2/tools/"
    cp "$RULES/verify-script-regressions.txt" "$RULES/numbers-policy.toml" \
       "$RULES/counters.toml" "$RULES/criteria.toml" "$d/v2/test/rules/"
    # đối chứng dương: verifier nguyên bản ⇒ self-test XANH (13/13 ca ĐỎ đúng mã).
    "$PY" "$d/v2/tools/verify-numbers.py" --root "$d" --self-test >/dev/null 2>&1 || {
        echo "    (self-test nguyên bản KHÔNG xanh)" >&2
        return 1
    }
    echo "    đối chứng dương: self-test XANH trên verifier nguyên bản"
    # phá: **làm yếu verifier** (miễn số thập phân `x.y` — đúng lỗi D2-117) ⇒ ca cũ xanh ⇒ ĐỎ.
    "$PY" - "$d" <<'PYEOF'
import sys
p = sys.argv[1] + "/v2/tools/verify-numbers.py"
s = open(p, encoding="utf-8").read()
needle = "ALLOW = [\n"
assert needle in s, "không thấy ALLOW"
s = s.replace(needle, needle + '    (re.compile(r"^\\d+\\.\\d+$"), "ca pha selfbreak: miem x.y"),\n', 1)
open(p, "w", encoding="utf-8").write(s)
PYEOF
    local out rc
    out="$("$PY" "$d/v2/tools/verify-numbers.py" --root "$d" --self-test 2>&1)"
    rc=$?
    if [ "$rc" -eq 0 ]; then
        echo "    (verifier bị làm yếu nhưng self-test vẫn XANH)" >&2
        return 1
    fi
    printf '%s\n' "$out" | grep -q "E8_CASE_GREEN" || {
        echo "    (self-test ĐỎ nhưng không phải vì ca cũ xanh → E8_CASE_GREEN)" >&2
        return 1
    }
    return 0
}

echo "TAP version 13"
echo "1..5"
check_case "luat 1: số dạng key=value ở prose ⇒ ĐỎ (E1_NUMBER_ASSIGNED)" case_law1
check_case "luat 2: báo cáo commit trước log ⇒ ĐỎ (E2_ORDER)" case_law2
check_case "luat 3: bộ đếm proven=false nằm trong khối ok ⇒ ĐỎ (E3_UNPROVEN_IN_OK)" case_law3
check_case "luat 7: bản trích bỏ một cột ⇒ ĐỎ (E7_COLUMN_MISMATCH)" case_law7
check_case "luat 8: verifier bị làm yếu ⇒ self-test ĐỎ (E8_CASE_GREEN)" case_law8

AFTER="$(hash_real_tree)"
if [ "$BEFORE" = "$AFTER" ]; then
    echo "ok - md5 cây thật KHÔNG đổi trước/sau ($BEFORE)"
else
    echo "not ok - md5 cây thật ĐÃ ĐỔI: trước=$BEFORE sau=$AFTER"
    fail=$((fail + 1))
fi
if [ "$fail" -ne 0 ]; then
    echo "# selfbreak: $fail lỗi / $total ca"
    exit 1
fi
echo "# selfbreak: $total/$total ca phá ĐỎ đúng mã; cây thật không đổi"
exit 0
