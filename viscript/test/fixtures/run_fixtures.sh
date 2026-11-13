#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Case âm bắt buộc của ST-01: mỗi luật của `vis-dev` phải CHỨNG MINH nó bắt được vi phạm.
set -u
VIS_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DEV="$VIS_ROOT/bin/vis-dev"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
echo "1..12"
n=0; bad=0
expect_fail() {
    n=$((n + 1))
    local name="$1" want="$2" dir="$3"
    if bash "$DEV" rules --root "$dir" >"$tmp/out" 2>&1; then
        echo "not ok $n - $name KHÔNG bị bắt"; bad=$((bad + 1))
    elif grep -q "$want" "$tmp/out"; then
        echo "ok $n - $name bị bắt ($want)"
    else
        echo "not ok $n - $name bị bắt nhưng sai lý do"; sed 's/^/    /' "$tmp/out"; bad=$((bad + 1))
    fi
}
expect_fail_dev() {
    n=$((n + 1))
    local name="$1" want="$2"
    shift 2
    if "$@" >"$tmp/out" 2>&1; then
        echo "not ok $n - $name KHÔNG bị bắt"; bad=$((bad + 1))
    elif grep -q "$want" "$tmp/out"; then
        echo "ok $n - $name bị bắt ($want)"
    else
        echo "not ok $n - $name bị bắt nhưng sai lý do"; sed 's/^/    /' "$tmp/out"; bad=$((bad + 1))
    fi
}
# qd fixtures (symlink) cho cây tạm: $1 = gốc, $2 = số lượng
mk_qd() {
    mkdir -p "$1/viscript/test/doc-examples"
    local i=0 f
    for f in "$VIS_ROOT"/test/doc-examples/qd*; do
        i=$((i + 1))
        [ "$i" -gt "$2" ] && break
        ln -sf "$f" "$1/viscript/test/doc-examples/"
    done
}
# 1) thiếu SPDX
mkdir -p "$tmp/1"; printf 'fn x() {}\n' > "$tmp/1/bad.rs"
expect_fail "thiếu SPDX" "R3" "$tmp/1"
# 2) tệp > 1.500 dòng
mkdir -p "$tmp/2"; { echo '// SPDX-License-Identifier: MIT OR Apache-2.0'; for i in $(seq 1 1501); do echo "// dòng $i"; done; } > "$tmp/2/long.rs"
expect_fail "tệp 1.501 dòng" "R1" "$tmp/2"
# 3) dùng unsafe
mkdir -p "$tmp/3"; printf '// SPDX-License-Identifier: MIT OR Apache-2.0\nunsafe fn x() {}\n' > "$tmp/3/u.rs"
expect_fail "unsafe" "R2" "$tmp/3"
# 4) vis-host-vifast nằm trong viscript/
mkdir -p "$tmp/4/vis-host-vifast"; printf '# SPDX-License-Identifier: MIT OR Apache-2.0\n' > "$tmp/4/vis-host-vifast/Cargo.toml"
expect_fail "vis-host-vifast trong viscript/" "R4" "$tmp/4"
# 5) môi trường sạch không có cargo: phải báo "không thấy cargo", không được 127
n=$((n + 1))
out=$(env -i HOME=/nonexistent PATH=/usr/bin:/bin bash "$VIS_ROOT/bin/vis" explain VS0107 2>&1)
rc=$?
if [ "$rc" -eq 127 ]; then
    echo "not ok $n - thiếu cargo vẫn nổ 127"; bad=$((bad + 1))
elif [ "$rc" -ne 0 ] && echo "$out" | grep -q "không thấy cargo"; then
    echo "ok $n - thiếu cargo báo rõ ($rc)"
else
    echo "not ok $n - thiếu cargo xử lý sai (rc=$rc): $out"; bad=$((bad + 1))
fi
# 6) doc-examples: khối ```vis chứa "..." KHÔNG khai báo allowlist ⇒ đỏ
mkdir -p "$tmp/6/docs/viscript"
printf '# tài liệu âm\n\n```vis\nfor i in 0..10: ...\n```\n' > "$tmp/6/docs/viscript/neg.md"
: > "$tmp/6/docs/viscript/doc-examples-allow.txt"
mk_qd "$tmp/6" 12
expect_fail_dev "doc-examples: khối đỏ ngoài allowlist" "khối đỏ không khai báo" \
    bash "$DEV" doc-examples --root "$tmp/6" --temp-root
# 7) allowlist cũ (khối đã xanh) ⇒ đỏ
mkdir -p "$tmp/7/docs/viscript"
printf '# tài liệu sạch\n\n```vis\nx = 1\n```\n' > "$tmp/7/docs/viscript/clean.md"
printf 'clean.md:3 lý do cũ (khối đã xanh)\n' > "$tmp/7/docs/viscript/doc-examples-allow.txt"
mk_qd "$tmp/7" 12
expect_fail_dev "doc-examples: ngoại lệ cũ đã xanh" "ngoại lệ cũ" \
    bash "$DEV" doc-examples --root "$tmp/7" --temp-root
# 8) allowlist sai số 7 (--root KHÔNG kèm --temp-root vẫn đủ luật) ⇒ đỏ
# **SỬA KÈM LÝ DO (ST-18)**: allowlist 7 → **9** mục (hai `vish-session` §9.7 dùng lệnh shell VS1),
# nên câu "sai số mục" phải khoá theo con số **hiện tại** — máy kiểm vẫn bắt đúng lỗi.
# **SỬA KÈM LÝ DO (ST-27, Agent 0 APPROVED)**: khối 662 (`extern cmd git(…, ...rest: str)`) nay
# **xanh** (variadic đã có) ⇒ allowlist còn **8** mục; câu "sai số mục" khoá theo con số hiện tại.
# **SỬA KÈM LÝ DO (ST-37, Agent 0 APPROVED `59a407c` + `260fe8a`, VD-70)**: **hai** khối `vish-session`
# §9.7 nay **xanh** — 1451 (`walk`/`where`/`sort_by`) và 672 (`ls ./photos | … | take 10`) — nhờ lệnh lạ
# = chương trình ngoài + khe ống **hợp nhất** (VD-69) ⇒ allowlist còn **6** mục; câu "sai số mục" khoá
# theo con số hiện tại (chặt hơn, không nới ngưỡng).
expect_fail_dev "doc-examples: allowlist sai số mục" "phải đúng 6 mục" \
    bash "$DEV" doc-examples --root "$tmp/7"
# 9) fixture lệch byte so với tài liệu ⇒ đỏ
mkdir -p "$tmp/9/docs/viscript" "$tmp/9/viscript/test/doc-examples"
cp "$VIS_ROOT"/../docs/viscript/*.md "$tmp/9/docs/viscript/"
cp "$VIS_ROOT"/../docs/viscript/doc-examples-allow.txt "$tmp/9/docs/viscript/"
cp "$VIS_ROOT"/test/doc-examples/doc-* "$tmp/9/viscript/test/doc-examples/"
mk_qd "$tmp/9" 12
printf '\n# lệch byte\n' >> "$tmp/9/viscript/test/doc-examples/doc-ViScript-thiet-ke-305.vis"
expect_fail_dev "doc-examples: fixture lệch byte" "KHÔNG khớp khối" \
    bash "$DEV" doc-examples --root "$tmp/9"
# 10) thiếu fixture qd (10) ⇒ đỏ (số do Agent 0 chốt: đúng 12)
mkdir -p "$tmp/10/docs/viscript"
printf '# tài liệu sạch\n\n```vis\nx = 1\n```\n' > "$tmp/10/docs/viscript/clean.md"
: > "$tmp/10/docs/viscript/doc-examples-allow.txt"
mk_qd "$tmp/10" 10
expect_fail_dev "doc-examples: qd-* = 10" "phải đúng 12 fixture qd" \
    bash "$DEV" doc-examples --root "$tmp/10" --temp-root
# 11) thừa fixture qd (13) ⇒ đỏ
mkdir -p "$tmp/11/docs/viscript"
cp "$tmp/10/docs/viscript/clean.md" "$tmp/11/docs/viscript/"
: > "$tmp/11/docs/viscript/doc-examples-allow.txt"
mk_qd "$tmp/11" 13
printf '# SPDX-License-Identifier: MIT OR Apache-2.0\n# QD 7.1: fixture thừa (case âm)\n# kỳ vọng: ok\nx = 1\n' \
    > "$tmp/11/viscript/test/doc-examples/qd-extra.vis"
expect_fail_dev "doc-examples: qd-* = 13" "phải đúng 12 fixture qd" \
    bash "$DEV" doc-examples --root "$tmp/11" --temp-root
# 12) binary `vis` cũ hơn mã nguồn ⇒ từ chối chạy (không dùng bản cũ)
mkdir -p "$tmp/12/docs/viscript"
cp "$tmp/10/docs/viscript/clean.md" "$tmp/12/docs/viscript/"
: > "$tmp/12/docs/viscript/doc-examples-allow.txt"
mk_qd "$tmp/12" 12
printf '#!/bin/sh\nexit 0\n' > "$tmp/12/fakevis"
chmod +x "$tmp/12/fakevis"
touch -t 200001010000 "$tmp/12/fakevis"
PY=$(command -v python3.12 || command -v python3.11 || command -v python3 || true)
expect_fail_dev "doc-examples: binary cũ bị từ chối" "cũ hơn mã nguồn" \
    "$PY" "$VIS_ROOT/test/doc-examples/doc_examples.py" --gate --root "$tmp/12" --temp-root --vis-bin "$tmp/12/fakevis"
if [ "$bad" -ne 0 ]; then echo "# fixtures: FAIL ($bad)"; exit 1; fi
echo "# fixtures: PASS (cây sạch phải xanh)"
