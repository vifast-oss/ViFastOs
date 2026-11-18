#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-145** — ca hồi quy cho §A4 của Agent 0 (phiếu lượt 5): **đường đo** phải TỪ CHỐI ảnh không
# cùng định nghĩa dấu vân tay (`algo=`); chỉ `status.sh` và `pin-artifact.sh` được nhận dạng cũ.
#
# Vì sao có tệp này: KT-143 nhúng `algo=` vào dòng `# build:` và cho `vbrev_parse` đòi `algo=`
# khớp định nghĩa hiện hành — nhưng **đường đo** vẫn còn ba chỗ nhận dạng cũ (`shell-edge.sh`,
# `pre-measure.sh`, và `soak.sh` chỉ kiểm dòng log chứ không kiểm **tệp ảnh**). Nghĩa là một ảnh
# build bằng định nghĩa dấu vân tay khác vẫn có thể lọt vào phép đo. Agent 0 phiếu lượt 5 §A4 đóng
# đúng lỗ ấy, và đòi **ca kiểm** + **đột biến nới ⇒ đỏ**.
#
# Hai phần, cả hai đều chạy trong vài giây (không QEMU, không build):
#
#   A. HÀNH VI — dựng ba ảnh giả trong thư mục tạm rồi gọi **chính** ba script đường đo:
#        · `ảnh dạng CŨ`  — có `inputs=`/`dirty=` nhưng **không** `algo=`  ⇒ phải TỪ CHỐI
#        · `ảnh algo SAI` — `algo=kt000-khong-co`, `inputs` **khớp hoàn hảo** cây hiện tại ⇒ phải TỪ CHỐI
#        · `ảnh algo ĐÚNG` — đối chứng **dương**: phải ĐƯỢC NHẬN (một phép kiểm luôn-từ-chối cũng
#          qua được hai ca âm, nên ca dương là ca giữ cho phép kiểm có nghĩa)
#      Ba script: `shell-edge.sh --check-image`, `soak.sh --check-image`, `pre-measure.sh`.
#
#   B. CẤU TRÚC — quét **toàn bộ** `v2/` (trừ `v2/agent/JOURNAL/`, nơi lưu log lịch sử) tìm lệnh
#      `grep` còn nhận dạng cũ của dòng `# build:`. Danh sách tệp được phép **chỉ** gồm:
#        · `v2/tools/status.sh`      — hiển thị ảnh lịch sử (Agent 0 cho phép)
#        · `v2/tools/pin-artifact.sh`— ghim ảnh lịch sử   (Agent 0 cho phép)
#        · `v2/test/boot/build-identity.sh` — `vbrev_image_identity` **phát hiện** dạng cũ để TỪ CHỐI
#      Thêm một tệp vào danh sách ⇒ phải sửa chính tệp này, tức phải nói lý do trong diff.
#
# Dùng:
#   bash v2/test/boot/algo-strict.sh            # chạy cả hai phần
#   bash v2/test/boot/algo-strict.sh --help
#
# Exit: 0 = mọi ca PASS; 1 = có ca FAIL; 2 = lỗi cấu hình.
set -u
LC_ALL=C
export LC_ALL

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/../.." && pwd)"
REPO_ROOT="$(cd "$V2_ROOT/.." && pwd)"

die() {
    echo "algo-strict: $*" >&2
    exit 2
}

case "${1:-}" in
    --help | -h)
        sed -n '2,/^set -u$/p' "${BASH_SOURCE[0]}" | sed '$d' | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    "") ;;
    *) die "tham số lạ: $1 (dùng --help)" ;;
esac

# shellcheck source=/dev/null
. "$HERE/build-identity.sh"

PASS=0
FAIL=0
ok() {
    PASS=$((PASS + 1))
    echo "# ok $1 - $2"
}
bad() {
    FAIL=$((FAIL + 1))
    echo "# not ok $1 - $2"
}

TMP="$(mktemp -d "${TMPDIR:-/tmp}/kt145-algo-strict.XXXXXX")" || die "mktemp"
trap 'rm -rf "$TMP"' EXIT

# --- dấu vân tay **thật** của cây hiện tại (không dùng giá trị giả) ---------------------------
vbrev_inputs_calc "$REPO_ROOT" >/dev/null 2>&1 || die "không tính được dấu vân tay cây hiện tại (${VBREV_CALC_ERROR:-?})"
CUR_FP="$VBREV_CALC_SHORT"
echo "# algo-strict: định nghĩa hiện hành algo=$VBREV_ALGO, dấu vân tay cây=$CUR_FP"

# --- ba ảnh giả ------------------------------------------------------------------------------
mk() { # <path> <dòng-danh-tính>
    { head -c 4096 /dev/zero; printf '%s\n' "$2"; } >"$1"
}
mk "$TMP/old.bin" "# build: inputs=$CUR_FP dirty=0"
mk "$TMP/badalgo.bin" "# build: algo=kt000-khong-co inputs=$CUR_FP dirty=0"
mk "$TMP/ok.bin" "# build: algo=$VBREV_ALGO inputs=$CUR_FP dirty=0"

# =============================================================================================
# PHẦN A — hành vi
# =============================================================================================
echo "# --- A. hành vi: ba script đường đo ---"

# expect_refuse <id> <script> <ảnh> <reason> [tham số...]
expect_refuse() {
    local id="$1" script="$2" img="$3" want="$4"
    shift 4
    local out rc
    out="$(bash "$script" "$@" "$img" 2>&1)"
    rc=$?
    if [ "$rc" -eq 0 ]; then
        bad "$id" "$(basename "$script") NHẬN ảnh phải bị từ chối (reason=$want) — [$out]"
        return
    fi
    case "$out" in
        *"REFUSE reason=$want"*) ok "$id" "$(basename "$script") TỪ CHỐI đúng reason=$want" ;;
        *) bad "$id" "$(basename "$script") từ chối nhưng SAI reason (mong $want) — [$out]" ;;
    esac
}

# expect_accept <id> <script> <ảnh> [tham số...]
expect_accept() {
    local id="$1" script="$2" img="$3"
    shift 3
    local out rc
    out="$(bash "$script" "$@" "$img" 2>&1)"
    rc=$?
    if [ "$rc" -ne 0 ]; then
        bad "$id" "$(basename "$script") TỪ CHỐI ảnh algo ĐÚNG (đối chứng dương hỏng) — [$out]"
        return
    fi
    case "$out" in
        *"ACCEPT"*) ok "$id" "$(basename "$script") NHẬN ảnh algo đúng" ;;
        *) bad "$id" "$(basename "$script") không in ACCEPT — [$out]" ;;
    esac
}

# (a) shell-edge.sh --check-image
expect_refuse a1 "$HERE/shell-edge.sh" "$TMP/old.bin" no-algo --check-image
expect_refuse a2 "$HERE/shell-edge.sh" "$TMP/badalgo.bin" algo-mismatch --check-image
expect_accept a3 "$HERE/shell-edge.sh" "$TMP/ok.bin" --check-image

# (b) soak.sh --check-image
expect_refuse b1 "$HERE/soak.sh" "$TMP/old.bin" no-algo --check-image
expect_refuse b2 "$HERE/soak.sh" "$TMP/badalgo.bin" algo-mismatch --check-image
expect_accept b3 "$HERE/soak.sh" "$TMP/ok.bin" --check-image

# (c) pre-measure.sh — không có `--check-image`; nó tự TỪ CHỐI trước khi đo.
#     Ảnh `ok.bin` sẽ bị từ chối ở bước **ghim** (`not-pinned`), nên ca dương ở đây đòi: **không**
#     bị từ chối vì lý do định nghĩa dấu vân tay. Đó đúng là điều cần chứng minh.
expect_refuse c1 "$V2_ROOT/tools/pre-measure.sh" "$TMP/old.bin" legacy-format
expect_refuse c2 "$V2_ROOT/tools/pre-measure.sh" "$TMP/badalgo.bin" algo-mismatch
out_c3="$(PRE_MEASURE_ART_DIR="$TMP/art" bash "$V2_ROOT/tools/pre-measure.sh" "$TMP/ok.bin" 2>&1)"
case "$out_c3" in
    *"REFUSE reason=legacy-format"* | *"REFUSE reason=algo-mismatch"* | *"REFUSE reason=no-algo"*)
        bad c3 "pre-measure.sh TỪ CHỐI ảnh algo ĐÚNG vì lý do định nghĩa dấu vân tay — [$out_c3]"
        ;;
    *)
        ok c3 "pre-measure.sh KHÔNG từ chối ảnh algo đúng vì lý do định nghĩa (đối chứng dương)"
        ;;
esac

# =============================================================================================
# PHẦN B — cấu trúc: quét toàn bộ phạm vi
# =============================================================================================
echo "# --- B. cấu trúc: quét toàn bộ v2/ tìm chỗ còn nhận dạng cũ ---"

# Danh sách **được phép** — mỗi dòng `<đường-dẫn-tương-đối-với-repo>|<lý do>`.
ALLOWED="v2/tools/status.sh|hiển thị ảnh lịch sử (Agent 0 §A4 cho phép)
v2/tools/pin-artifact.sh|ghim ảnh lịch sử (Agent 0 §A4 cho phép)
v2/test/boot/build-identity.sh|vbrev_image_identity PHÁT HIỆN dạng cũ để TỪ CHỐI, không nhận"

# Mẫu nhận dạng cũ: `# build: inputs=`, `# build: rev=`, `# build: [a-z]*=` nằm trong một lệnh grep.
# Quét **mọi** tệp `.sh` dưới `v2/`, trừ `v2/agent/JOURNAL/` (log lịch sử, đọc-only) và chính tệp này.
SCAN_HITS="$(cd "$REPO_ROOT" && grep -rn --include=*.sh -E "grep.*# build: (inputs=|rev=|\(inputs\|rev\)=|\[a-z\])" v2/ 2>/dev/null |
    grep -v '^v2/agent/JOURNAL/' |
    grep -v '^v2/test/boot/algo-strict.sh:' || true)"

SCAN_FILES="$(printf '%s\n' "$SCAN_HITS" | sed -n 's/^\([^:]*\):.*/\1/p' | sort -u)"
ALLOWED_FILES="$(printf '%s\n' "$ALLOWED" | sed -n 's/^\([^|]*\)|.*/\1/p' | sort -u)"

# (d) mọi tệp còn nhận dạng cũ phải nằm trong danh sách được phép
UNEXPECTED="$(comm -23 <(printf '%s\n' "$SCAN_FILES") <(printf '%s\n' "$ALLOWED_FILES") | sed '/^$/d')"
if [ -z "$UNEXPECTED" ]; then
    ok d1 "không tệp nào NGOÀI danh sách được phép còn nhận dạng cũ ($(printf '%s\n' "$SCAN_FILES" | sed '/^$/d' | wc -l | tr -d ' ') tệp trong danh sách)"
else
    bad d1 "tệp còn nhận dạng cũ mà KHÔNG được phép: $(printf '%s' "$UNEXPECTED" | tr '\n' ' ')"
fi

# (d2) mỗi tệp trong danh sách được phép phải **thật sự** còn khớp mẫu — nếu không, danh sách
#      đang mục rữa (một mục "được phép" mà không còn gì để cho phép là dấu hiệu tệp đã đổi mà
#      danh sách không theo). Ca này giữ cho (d1) không trở thành "danh sách càng dài càng an toàn".
STALE=""
while IFS= read -r f; do
    [ -n "$f" ] || continue
    printf '%s\n' "$SCAN_FILES" | grep -qxF "$f" || STALE="$STALE $f"
done <<EOF
$ALLOWED_FILES
EOF
if [ -z "$STALE" ]; then
    ok d2 "mọi mục trong danh sách được phép đều còn thật sự khớp mẫu (danh sách không mục rữa)"
else
    bad d2 "mục trong danh sách được phép KHÔNG còn khớp mẫu:$STALE"
fi

# (e) ba script đường đo **không** được nằm trong danh sách được phép
for f in v2/test/boot/shell-edge.sh v2/test/boot/soak.sh v2/tools/pre-measure.sh v2/tools/repro-build.sh; do
    if printf '%s\n' "$ALLOWED_FILES" | grep -qxF "$f"; then
        bad "e:$f" "$f nằm trong danh sách được phép nhận dạng cũ — đường đo KHÔNG được phép"
    else
        ok "e:$f" "$f không nằm trong danh sách được phép nhận dạng cũ"
    fi
done

# =============================================================================================
echo "# algo-strict: $PASS PASS, $FAIL FAIL"
if [ "$FAIL" -eq 0 ]; then
    echo "# algo-strict.sh: PASS"
    exit 0
fi
echo "# algo-strict.sh: FAIL"
exit 1
