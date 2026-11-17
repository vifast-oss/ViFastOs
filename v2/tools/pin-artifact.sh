#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-84 (D2-122.6) — **ghim** một ảnh đã build vào `v2/artifacts/<sha256>.bin`.
#
# Vì sao: mọi phép đo phải trỏ vào **một tệp bất biến**, không trỏ vào `target/…` (nơi lần build
# sau ghi đè). Tên tệp **là** sha256 ⇒ không thể "ghim nhầm ảnh": đọc tên là biết nội dung.
#
# Dùng:
#   bash v2/tools/pin-artifact.sh <ảnh>            # ghim vào v2/artifacts/
#   bash v2/tools/pin-artifact.sh <ảnh> --dir DIR  # ghim vào DIR
#   ... --allow-stale   # cho phép ảnh có dấu vân tay KHÁC cây hiện tại (mặc định: TỪ CHỐI)
#   ... --allow-dirty   # cho phép ảnh tự khai `dirty=1` (mặc định: TỪ CHỐI)
#   bash v2/tools/pin-artifact.sh --verify [--dir DIR]   # kiểm **mọi** ảnh đã ghim
#   bash v2/tools/pin-artifact.sh --selftest              # tự kiểm 6 ca (dương + âm)
#
# **Vì sao kiểm dấu vân tay**: ghim một ảnh build từ cây khác là đúng lớp lỗi D2-108A (đo ảnh A,
# trích số từ ảnh B). Mặc định script **từ chối** ảnh lệch cây hiện tại.
#
# Exit: 0 = đã ghim (mới hoặc đã có, byte giống); 2 = dùng sai; 3 = ảnh không có dòng `# build:`;
#       4 = tệp đích đã tồn tại nhưng **khác** byte (không ghi đè — fail-closed);
#       5 = dấu vân tay ảnh KHÁC cây hiện tại; 6 = ảnh tự khai `dirty=1`.
set -u
LC_ALL=C
export LC_ALL

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$V2_ROOT/.." && pwd)"
DEST_DIR="$V2_ROOT/artifacts"

die() {
    echo "pin-artifact: $*" >&2
    exit 2
}

[ "$#" -ge 1 ] || die "dùng: pin-artifact.sh <ảnh> [--dir DIR] | --verify [--dir DIR]"

# --- chế độ --selftest: dựng ảnh giả trong thư mục tạm, kiểm cả nhánh nhận và nhánh từ chối
if [ "$1" = "--selftest" ]; then
    TMP="$(mktemp -d "${TMPDIR:-/tmp}/pin-selftest.XXXXXX")" || die "mktemp"
    trap 'rm -rf "$TMP"' EXIT
    rc=0
    # Ảnh giả: 4 KiB rác + dòng danh tính (đúng dạng kernel in).
    mk_img() { # <path> <inputs> <dirty>
        { head -c 4096 /dev/zero; printf '# build: inputs=%s dirty=%s\n' "$2" "$3"; } >"$1"
    }
    # Dấu vân tay **thật** của cây hiện tại (để ca dương khớp); nếu không tính được thì dùng
    # giá trị cố định và ca dương sẽ bị từ chối — đúng fail-closed, và selftest báo FAIL.
    CUR_FP="0123456789abcdef"
    if [ -f "$V2_ROOT/test/boot/build-rev.sh" ]; then
        # shellcheck disable=SC1091
        . "$V2_ROOT/test/boot/build-rev.sh"
        vbrev_inputs_calc "$ROOT" >/dev/null 2>&1 && CUR_FP="$VBREV_CALC_SHORT"
    fi
    mk_img "$TMP/good.bin" "$CUR_FP" 0
    mk_img "$TMP/dirty.bin" "$CUR_FP" 1
    mk_img "$TMP/stale.bin" "ffffffffffffffff" 0
    head -c 4096 /dev/zero >"$TMP/nobuild.bin"

    # (a) ảnh hợp lệ, khớp cây hiện tại ⇒ ghim được
    if ! bash "$0" "$TMP/good.bin" --dir "$TMP/out" >/dev/null 2>&1; then
        echo "# pin-selftest (a) FAIL: ảnh hợp lệ bị từ chối"; rc=1
    fi
    # (b) ghim lại ⇒ state=da-co
    if ! bash "$0" "$TMP/good.bin" --dir "$TMP/out" 2>/dev/null | grep -q 'state=da-co'; then
        echo "# pin-selftest (b) FAIL: ghim lại không ra state=da-co"; rc=1
    fi
    # (c) ảnh không có dòng build ⇒ mã thoát ba
    bash "$0" "$TMP/nobuild.bin" --dir "$TMP/out" >/dev/null 2>&1
    [ "$?" -eq 3 ] || { echo "# pin-selftest (c) FAIL: ảnh thiếu dòng build không trả mã ba"; rc=1; }
    # (d) ảnh dirty ⇒ mã thoát sáu
    bash "$0" "$TMP/dirty.bin" --dir "$TMP/out" >/dev/null 2>&1
    [ "$?" -eq 6 ] || { echo "# pin-selftest (d) FAIL: ảnh dirty không trả mã sáu"; rc=1; }
    # (e) ảnh lệch dấu vân tay ⇒ mã thoát năm
    bash "$0" "$TMP/stale.bin" --dir "$TMP/out" >/dev/null 2>&1
    [ "$?" -eq 5 ] || { echo "# pin-selftest (e) FAIL: ảnh lệch dấu vân tay không trả mã năm"; rc=1; }
    # (f) --verify bắt tên sai ⇒ mã thoát bảy
    cp "$TMP/good.bin" "$TMP/out/0000000000000000000000000000000000000000000000000000000000000000.bin"
    bash "$0" --verify --dir "$TMP/out" >/dev/null 2>&1
    [ "$?" -eq 7 ] || { echo "# pin-selftest (f) FAIL: --verify không bắt tên sai"; rc=1; }
    if [ "$rc" -eq 0 ]; then
        echo "# pin-selftest: PASS (a/b/c/d/e/f)"
    else
        echo "# pin-selftest: FAIL"
    fi
    exit "$rc"
fi

# --- chế độ --verify: mọi tệp trong DEST_DIR phải (a) tên = sha256 của nội dung, (b) có dòng build
if [ "$1" = "--verify" ]; then
    shift
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --dir)
                [ "$#" -ge 2 ] || die "--dir cần tham số"
                DEST_DIR="$2"
                shift 2
                ;;
            *) die "tham số lạ: $1" ;;
        esac
    done
    [ -d "$DEST_DIR" ] || die "không thấy thư mục $DEST_DIR"
    n_ok=0
    n_bad=0
    n_legacy=0
    for f in "$DEST_DIR"/*.bin; do
        [ -f "$f" ] || continue
        base="$(basename "$f" .bin)"
        got="$(shasum -a 256 "$f" | awk '{print $1}')"
        if [ "$base" != "$got" ]; then
            echo "VERIFY: SAI TÊN $f (tên=$base, sha=$got)" >&2
            n_bad=$((n_bad + 1))
            continue
        fi
        # Chấp nhận **cả hai** định dạng danh tính: `inputs=` (từ KT-89) và `rev=` (trước KT-89).
        # Ảnh **trước KT-86** không có dòng nào — đó là ảnh lịch sử, đếm riêng là `legacy`,
        # **không** phải lỗi (tên tệp vẫn phải khớp sha, đó mới là lỗi).
        if ! LC_ALL=C grep -a -q -m1 -e '# build: inputs=' -e '# build: rev=' "$f"; then
            n_legacy=$((n_legacy + 1))
            continue
        fi
        n_ok=$((n_ok + 1))
    done
    echo "VERIFY: ok=$n_ok bad=$n_bad legacy=$n_legacy dir=$DEST_DIR"
    [ "$n_bad" -eq 0 ] || exit 7
    exit 0
fi

IMAGE="$1"
shift
ALLOW_STALE=0
ALLOW_DIRTY=0
while [ "$#" -gt 0 ]; do
    case "$1" in
        --dir)
            [ "$#" -ge 2 ] || die "--dir cần tham số"
            DEST_DIR="$2"
            shift 2
            ;;
        --allow-stale) ALLOW_STALE=1; shift ;;
        --allow-dirty) ALLOW_DIRTY=1; shift ;;
        *) die "tham số lạ: $1" ;;
    esac
done

[ -f "$IMAGE" ] || die "không thấy ảnh: $IMAGE"
mkdir -p "$DEST_DIR" || die "không tạo được $DEST_DIR"

SHA="$(shasum -a 256 "$IMAGE" | awk '{print $1}')"
SIZE="$(wc -c <"$IMAGE" | tr -d ' ')"
DEST="$DEST_DIR/$SHA.bin"

# Ảnh phải tự khai nguồn (KT-86/KT-89): không có dòng `# build:` ⇒ không ghim.
LINE="$(LC_ALL=C grep -a -o -m1 '# build: inputs=[0-9a-f]* dirty=[01]' "$IMAGE" 2>/dev/null || true)"
[ -n "$LINE" ] || {
    echo "pin-artifact: ảnh KHÔNG có dòng '# build: inputs=… dirty=…' — không ghim (fail-closed)" >&2
    exit 3
}

# --- Kiểm ảnh có đúng **cây hiện tại** không (D2-108A) --------------------------------------
IMG_INPUTS="$(printf '%s' "$LINE" | sed -n 's/.*inputs=\([0-9a-f]*\).*/\1/p')"
IMG_DIRTY="$(printf '%s' "$LINE" | sed -n 's/.*dirty=\([01]\).*/\1/p')"
if [ "$IMG_DIRTY" = "1" ] && [ "$ALLOW_DIRTY" -eq 0 ]; then
    echo "pin-artifact: ảnh tự khai dirty=1 (build từ cây bẩn) — không ghim (dùng --allow-dirty nếu cố ý)" >&2
    exit 6
fi
# shellcheck source=/dev/null
if [ -f "$V2_ROOT/test/boot/build-rev.sh" ]; then
    # shellcheck disable=SC1091
    . "$V2_ROOT/test/boot/build-rev.sh"
    if vbrev_inputs_calc "$ROOT" >/dev/null 2>&1; then
        CUR="$VBREV_CALC_SHORT"
        if [ "$IMG_INPUTS" != "$CUR" ] && [ "$ALLOW_STALE" -eq 0 ]; then
            echo "pin-artifact: dấu vân tay ảnh ($IMG_INPUTS) KHÁC cây hiện tại ($CUR) — không ghim" >&2
            echo "pin-artifact: build lại từ HEAD rồi ghim, hoặc dùng --allow-stale nếu cố ý giữ ảnh cũ" >&2
            exit 5
        fi
    else
        echo "pin-artifact: CẢNH BÁO không tính được dấu vân tay cây hiện tại — bỏ qua kiểm tươi" >&2
    fi
fi

if [ -f "$DEST" ]; then
    OLD="$(shasum -a 256 "$DEST" | awk '{print $1}')"
    [ "$OLD" = "$SHA" ] || {
        echo "pin-artifact: $DEST đã tồn tại nhưng KHÁC byte (sha=$OLD) — không ghi đè" >&2
        exit 4
    }
    echo "PIN sha=$SHA size=$SIZE path=$DEST $LINE state=da-co"
    exit 0
fi

cp "$IMAGE" "$DEST" || die "không chép được vào $DEST"
NEW="$(shasum -a 256 "$DEST" | awk '{print $1}')"
[ "$NEW" = "$SHA" ] || die "sau khi chép, sha đổi ($NEW != $SHA) — không dùng"
echo "PIN sha=$SHA size=$SIZE path=$DEST $LINE state=moi"
