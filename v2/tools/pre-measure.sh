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
#   bash v2/tools/pre-measure.sh --selftest     # tự kiểm 7 ca (dương + âm)
#
# **KT-145** (Agent 0 phiếu lượt 5 §A4): cờ `--allow-legacy` đã **bị bỏ**. Trước đây nó cho phép
# một phép đo chấp nhận ảnh định dạng CŨ (`rev=`, thiếu `algo=`) — tức đúng cái lỗ mà §A4 đóng.
# Nay **đường đo** chỉ nhận ảnh tự khai ĐÚNG định nghĩa dấu vân tay hiện hành; chỉ
# `v2/tools/status.sh` và `v2/tools/pin-artifact.sh` được nhận dạng cũ (hiển thị/ghim ảnh lịch sử).
#
# Thư mục ảnh ghim đổi được bằng `PRE_MEASURE_ART_DIR` (mặc định `v2/artifacts`) — `--selftest`
# dùng nó để không đụng ảnh thật. **Hành vi mặc định không đổi.**
#
# Exit: 0 = ĐO ĐƯỢC; 2 = dùng sai; 3 = ảnh không có dòng danh tính; 4 = dấu vân tay ảnh ≠ cây
#       (hoặc ảnh dạng cũ / `algo=` khác định nghĩa hiện hành);
#       5 = ảnh tự khai dirty; 6 = ảnh chưa ghim vào artifacts/; 7 = cây nguồn bẩn; 8 = QEMU sót.
set -u
LC_ALL=C
export LC_ALL

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$V2_ROOT/.." && pwd)"
ART_DIR="${PRE_MEASURE_ART_DIR:-$V2_ROOT/artifacts}"
QUIET=0

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

[ "$#" -ge 1 ] || die "dùng: pre-measure.sh <ảnh> [--quiet] | --selftest"

# --- chế độ --selftest: dựng ảnh giả trong thư mục tạm, kiểm cả nhánh nhận và nhánh từ chối ----
# Mọi lần gọi con đều đặt `PRE_MEASURE_ART_DIR` vào thư mục tạm ⇒ **không** đụng `v2/artifacts/`.
if [ "$1" = "--selftest" ]; then
    TMP="$(mktemp -d "${TMPDIR:-/tmp}/pre-measure-selftest.XXXXXX")" || die "mktemp"
    trap 'rm -rf "$TMP"' EXIT
    rc=0
    ART="$TMP/art" # ART_DIR giả (thay cho v2/artifacts)
    mkdir -p "$ART" "$TMP/bin"
    # Máy có thể đang chạy QEMU của task khác ⇒ phép kiểm QEMU (kiểm **môi trường**, không phải
    # kiểm ảnh) được trung hoà bằng một `pgrep` giả **chỉ** trong PATH của các lần gọi dưới đây.
    printf '#!/bin/sh\nexit 1\n' >"$TMP/bin/pgrep"
    chmod +x "$TMP/bin/pgrep"

    # Ảnh giả: vài KiB rác + dòng danh tính (đúng dạng kernel in).
    mk_img() { # <path> <inputs> <dirty> [kích thước rác]
        # KT-143: dòng danh tính nay có `algo=` (tên định nghĩa dấu vân tay) — ảnh giả phải đúng
        # dạng kernel in, nếu không các ca tự kiểm sẽ đo một định dạng không tồn tại.
        { head -c "${4:-4096}" /dev/zero; printf '# build: algo=%s inputs=%s dirty=%s\n' "$VBREV_ALGO" "$2" "$3"; } >"$1"
    }
    mk_legacy() { # <path> <rev> <dirty>
        { head -c 4096 /dev/zero; printf '# build: rev=%s dirty=%s\n' "$2" "$3"; } >"$1"
    }
    # KT-145: ảnh **dạng mới** nhưng `algo=` KHÁC định nghĩa hiện hành — ca âm cho §A4.
    mk_badalgo() { # <path> <inputs> <dirty>
        { head -c 4096 /dev/zero; printf '# build: algo=kt000-khong-co inputs=%s dirty=%s\n' "$2" "$3"; } >"$1"
    }
    pin() { # <ảnh> — chép vào ART dưới tên sha256 (cùng quy ước pin-artifact.sh)
        local sha
        sha="$(shasum -a 256 "$1" | awk '{print $1}')"
        cp "$1" "$ART/$sha.bin"
    }
    pm() { # <ảnh> [tham số...] -> PM_OUT, PM_RC
        PM_OUT="$(PATH="$TMP/bin:$PATH" PRE_MEASURE_ART_DIR="$ART" bash "$0" "$@" 2>&1)"
        PM_RC=$?
    }
    expect_refuse() { # <ca> <mã> <lý do> <ảnh> [tham số...]
        local id="$1" want_rc="$2" want_reason="$3"
        shift 3
        pm "$@"
        if [ "$PM_RC" -ne "$want_rc" ]; then
            echo "# pre-measure-selftest ($id) FAIL: mã thoát $PM_RC (mong $want_rc)"
            rc=1
        elif ! printf '%s\n' "$PM_OUT" | grep -qF "REFUSE reason=$want_reason"; then
            echo "# pre-measure-selftest ($id) FAIL: không thấy 'REFUSE reason=$want_reason'"
            rc=1
        fi
    }
    expect_ok() { # <ca> <ảnh> [tham số...]
        local id="$1"
        shift
        pm "$@"
        if [ "$PM_RC" -ne 0 ]; then
            echo "# pre-measure-selftest ($id) FAIL: mã thoát $PM_RC (mong 0 = OK)"
            rc=1
        elif ! printf '%s\n' "$PM_OUT" | grep -qF 'PRE-MEASURE: OK'; then
            echo "# pre-measure-selftest ($id) FAIL: không thấy 'PRE-MEASURE: OK'"
            rc=1
        fi
    }

    # Dấu vân tay **thật** của cây hiện tại — ca (a) phải khớp cây thật, không dùng giá trị giả.
    CUR_FP=""
    if [ -f "$V2_ROOT/test/boot/build-rev.sh" ]; then
        # shellcheck disable=SC1091
        . "$V2_ROOT/test/boot/build-rev.sh"
        vbrev_inputs_calc "$ROOT" >/dev/null 2>&1 && CUR_FP="$VBREV_CALC_SHORT"
    fi
    if [ -z "$CUR_FP" ]; then
        # Không tính được ⇒ ca (a) không thể đúng; báo đỏ ngay thay vì dùng giá trị giả để "cho qua".
        echo "# pre-measure-selftest (a) FAIL: không tính được dấu vân tay cây hiện tại (fail-closed)"
        echo "# pre-measure-selftest: FAIL"
        exit 1
    fi

    mk_img "$TMP/good.bin" "$CUR_FP" 0
    mk_img "$TMP/dirty.bin" "$CUR_FP" 1
    mk_img "$TMP/stale.bin" "ffffffffffffffff" 0
    mk_img "$TMP/unpinned.bin" "$CUR_FP" 0 8192 # khác byte với good.bin ⇒ chưa ghim
    head -c 4096 /dev/zero >"$TMP/nobuild.bin"
    mk_legacy "$TMP/legacy.bin" "cf96eb9b589d" 0
    mk_badalgo "$TMP/badalgo.bin" "$CUR_FP" 0
    pin "$TMP/good.bin"
    pin "$TMP/legacy.bin"
    pin "$TMP/badalgo.bin"

    expect_ok a "$TMP/good.bin"                         # (a) khớp cây + đã ghim + cây sạch
    expect_refuse b 4 inputs-mismatch "$TMP/stale.bin"  # (b) lệch dấu vân tay
    expect_refuse c 5 image-dirty "$TMP/dirty.bin"      # (c) ảnh tự khai dirty=1
    expect_refuse d 6 not-pinned "$TMP/unpinned.bin"    # (d) ảnh chưa ghim
    expect_refuse e 3 no-build-line "$TMP/nobuild.bin"  # (e) ảnh không có dòng build
    expect_refuse f 4 legacy-format "$TMP/legacy.bin"   # (f) định dạng cũ ⇒ từ chối
    # (g) KT-145: `algo=` khác định nghĩa hiện hành ⇒ TỪ CHỐI, **kể cả** khi `inputs` khớp hoàn hảo
    #     (ảnh dưới đây mang đúng `$CUR_FP` của cây). Ca này ĐỎ nếu ai đó bỏ phép kiểm `algo`.
    expect_refuse g 4 algo-mismatch "$TMP/badalgo.bin"

    if [ "$rc" -eq 0 ]; then
        echo "# pre-measure-selftest: PASS (a/b/c/d/e/f/g)"
    else
        echo "# pre-measure-selftest: FAIL"
    fi
    exit "$rc"
fi

IMAGE="$1"
shift
while [ "$#" -gt 0 ]; do
    case "$1" in
        --quiet) QUIET=1; shift ;;
        *) die "tham số lạ: $1" ;;
    esac
done
[ -f "$IMAGE" ] || die "không thấy ảnh: $IMAGE"

# --- 1. danh tính ảnh ------------------------------------------------------------------------
SHA="$(shasum -a 256 "$IMAGE" | awk '{print $1}')"
SIZE="$(wc -c <"$IMAGE" | tr -d ' ')"
# shellcheck source=/dev/null
. "$V2_ROOT/test/boot/build-identity.sh"
# **KT-145** (§A4): kiểm **định nghĩa** dấu vân tay **trước** mọi phép đọc khác. Thứ tự này quan
# trọng vì hai lẽ: (i) đổi định nghĩa làm `inputs` đổi mà không đổi byte mã nào, nên một ảnh cũ có
# thể "tình cờ" khớp `inputs` với định nghĩa mới — so giá trị trước sẽ nhận nhầm; (ii) ảnh **dạng
# cũ** phải báo đúng `legacy-format`, không bị nhầm thành `no-build-line` (nó **có** dòng danh
# tính, chỉ là dạng cũ) — hai nguyên nhân khác nhau, người đọc cần biết mình đang cầm ảnh gì.
if ! vbrev_image_identity "$IMAGE"; then
    case "$VBREV_IMG_REASON" in
        no-build-line) refuse 3 no-build-line ;;   # không có dòng danh tính nào (mã thoát cũ, giữ nguyên)
        no-algo) refuse 4 legacy-format ;;         # CÓ dòng nhưng dạng cũ (thiếu `algo=`)
        *) refuse 4 "$VBREV_IMG_REASON" ;;
    esac
fi
LINE="# build: algo=$VBREV_IMG_ALGO inputs=$VBREV_IMG_INPUTS dirty=$VBREV_IMG_DIRTY"
say "# ảnh: sha256=$SHA size=$SIZE"
say "# danh tính: $LINE"
say "# định nghĩa dấu vân tay: ảnh=$VBREV_IMG_ALGO hiện hành=$VBREV_ALGO"

# --- 2. dấu vân tay so với cây hiện tại ------------------------------------------------------
HEAD="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || true)"
[ -n "$HEAD" ] || refuse 7 no-git
say "# HEAD: $HEAD"
IMG_INPUTS="$VBREV_IMG_INPUTS"
IMG_DIRTY="$VBREV_IMG_DIRTY"
vbrev_inputs_calc "$ROOT" >/dev/null 2>&1 || refuse 4 no-current-fingerprint
CUR="$VBREV_CALC_SHORT"
say "# dấu vân tay: ảnh=$IMG_INPUTS cây=$CUR"
[ "$IMG_INPUTS" = "$CUR" ] || refuse 4 inputs-mismatch
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
# KT-100 (review đêm 17/11): thiếu `pgrep` thì pipeline ra 0 ⇒ bước này **im lặng PASS**.
# Nay: không có `pgrep` ⇒ TỪ CHỐI (không chứng minh được "không có QEMU sót").
command -v pgrep >/dev/null 2>&1 || refuse 8 no-pgrep
QEMU_N="$(pgrep -f qemu-system 2>/dev/null | wc -l | tr -d ' ')"
say "# qemu đang chạy: $QEMU_N"
[ "$QEMU_N" -eq 0 ] || refuse 8 qemu-orphans

echo "PRE-MEASURE: OK sha256=$SHA size=$SIZE head=$HEAD inputs=${IMG_INPUTS:-rev-format} pinned=$PINNED"
