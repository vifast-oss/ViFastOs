#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-95** — kiểm trước khi đo: ảnh có đúng cây hiện tại không, có ghim không, máy có sạch không.
#
# Vì sao có script này: trong một phiên đã xảy ra **năm** lần cùng một lớp lỗi (D2-108A) — đo một
# ảnh nhưng trích số từ ảnh khác, hoặc đo ảnh build từ cây khác. Mỗi lần đều phát hiện **sau** khi
# đo xong. Script này gom các phép kiểm rẻ tiền vào **một** chỗ, chạy trước mọi phép đo.
#
# Dùng:
#   bash v2/tools/pre-measure.sh <ảnh>          # kiểm và in bảng trạng thái
#   bash v2/tools/pre-measure.sh <ảnh> --quiet  # chỉ in dòng kết luận
#   ... --allow-legacy   # chấp nhận ảnh định dạng CŨ (`rev=`, trước KT-89) — chỉ cho ảnh lịch sử
#
# Exit: 0 = ĐO ĐƯỢC; 2 = dùng sai; 3 = ảnh không có dòng danh tính; 4 = dấu vân tay ảnh ≠ cây;
#       5 = ảnh tự khai dirty; 6 = ảnh chưa ghim vào artifacts/; 7 = cây nguồn bẩn; 8 = QEMU sót.
set -u
LC_ALL=C
export LC_ALL

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$V2_ROOT/.." && pwd)"
ART_DIR="$V2_ROOT/artifacts"
QUIET=0
ALLOW_LEGACY=0

die() {
    echo "pre-measure: $*" >&2
    exit 2
}
say() {
    [ "$QUIET" -eq 1 ] || echo "$*"
}
refuse() { # <mã> <lý do>
    echo "PRE-MEASURE: REFUSE reason=$2"
    exit "$1"
}

[ "$#" -ge 1 ] || die "dùng: pre-measure.sh <ảnh> [--quiet]"
IMAGE="$1"
shift
while [ "$#" -gt 0 ]; do
    case "$1" in
        --quiet) QUIET=1; shift ;;
        --allow-legacy) ALLOW_LEGACY=1; shift ;;
        *) die "tham số lạ: $1" ;;
    esac
done
[ -f "$IMAGE" ] || die "không thấy ảnh: $IMAGE"

# --- 1. danh tính ảnh ------------------------------------------------------------------------
SHA="$(shasum -a 256 "$IMAGE" | awk '{print $1}')"
SIZE="$(wc -c <"$IMAGE" | tr -d ' ')"
LINE="$(LC_ALL=C grep -a -o -m1 -e '# build: inputs=[0-9a-f]* dirty=[01]' -e '# build: rev=[0-9a-f]* dirty=[01]' "$IMAGE" 2>/dev/null || true)"
[ -n "$LINE" ] || refuse 3 no-build-line
say "# ảnh: sha256=$SHA size=$SIZE"
say "# danh tính: $LINE"

# --- 2. dấu vân tay so với cây hiện tại ------------------------------------------------------
HEAD="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || true)"
[ -n "$HEAD" ] || refuse 7 no-git
say "# HEAD: $HEAD"

IMG_INPUTS="$(printf '%s' "$LINE" | sed -n 's/.*inputs=\([0-9a-f]*\).*/\1/p')"
IMG_DIRTY="$(printf '%s' "$LINE" | sed -n 's/.*dirty=\([01]\).*/\1/p')"
if [ -n "$IMG_INPUTS" ]; then
    # shellcheck source=/dev/null
    . "$V2_ROOT/test/boot/build-rev.sh"
    vbrev_inputs_calc "$ROOT" >/dev/null 2>&1 || refuse 4 no-current-fingerprint
    CUR="$VBREV_CALC_SHORT"
    say "# dấu vân tay: ảnh=$IMG_INPUTS cây=$CUR"
    [ "$IMG_INPUTS" = "$CUR" ] || refuse 4 inputs-mismatch
else
    # Ảnh định dạng cũ **không** chứng minh được nó khớp cây hiện tại ⇒ mặc định TỪ CHỐI.
    say "# dấu vân tay: ảnh dùng định dạng CŨ (rev=) — không so được với cây"
    [ "$ALLOW_LEGACY" -eq 1 ] || refuse 4 legacy-format
fi
[ "$IMG_DIRTY" = "1" ] && refuse 5 image-dirty

# --- 3. ảnh đã ghim chưa ---------------------------------------------------------------------
PINNED="$ART_DIR/$SHA.bin"
if [ -f "$PINNED" ]; then
    say "# ghim: có ($PINNED)"
else
    refuse 6 not-pinned
fi

# --- 4. cây nguồn sạch (phạm vi build-inputs) ------------------------------------------------
# shellcheck source=/dev/null
. "$V2_ROOT/test/boot/build-rev.sh"
DIRTY="$(vbrev_dirty "$ROOT" 2>/dev/null || echo 1)"
say "# cây nguồn (build-inputs): dirty=$DIRTY"
[ "$DIRTY" = "0" ] || refuse 7 tree-dirty

# --- 5. QEMU sót -----------------------------------------------------------------------------
QEMU_N="$(pgrep -f qemu-system 2>/dev/null | wc -l | tr -d ' ')"
say "# qemu đang chạy: $QEMU_N"
[ "$QEMU_N" -eq 0 ] || refuse 8 qemu-orphans

echo "PRE-MEASURE: OK sha256=$SHA size=$SIZE head=$HEAD inputs=${IMG_INPUTS:-rev-format} pinned=$PINNED"
