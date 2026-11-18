#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# **KT-145** — cổng **danh tính ảnh** cho đường đo: ảnh phải tự khai ĐÚNG **định nghĩa** dấu vân
# tay hiện hành (`algo=`), không chỉ đúng **giá trị** (`inputs=`).
#
# Vì sao tệp này **tách khỏi** `build-rev.sh`: `build-rev.sh` là **định nghĩa** dấu vân tay và được
# ghim bằng `v2/tools/check-algo-pin.sh` (đổi tệp ấy ⇒ phải tăng `VBREV_ALGO`, vì giá trị dấu vân
# tay đổi theo). Hàm dưới đây là **người tiêu thụ** định nghĩa ấy, không tham gia vào phép băm —
# thêm nó vào `build-rev.sh` sẽ buộc phải tăng `VBREV_ALGO` và làm mọi ảnh/bằng chứng đã ghi lệch
# mốc, trong khi thuật toán băm **không đổi một byte**. Tách tệp giữ đúng cả hai: định nghĩa bất
# động, cổng mới có chỗ ở.
#
# **Vì sao cần cổng này** (Agent 0 phiếu lượt 5 §A4): KT-143 đã nhúng `algo=` vào dòng `# build:`
# và cho `vbrev_parse` đòi `algo=` khớp — nhưng **đường đo** vẫn còn chỗ nhận dạng cũ. Một ảnh
# build bằng định nghĩa dấu vân tay khác có thể "tình cờ" khớp `inputs` với định nghĩa mới, và khi
# ấy phép đo xanh trên một ảnh **không phải** ảnh mà định nghĩa hiện hành mô tả.
#
# **Ai được nhận dạng cũ**: chỉ `v2/tools/status.sh` (hiển thị ảnh lịch sử) và
# `v2/tools/pin-artifact.sh` (ghim ảnh lịch sử). **Đường đo** (`pre-measure.sh`, `soak.sh`,
# `shell-edge.sh`, `repro-build.sh`) phải TỪ CHỐI. Ca hồi quy:
# `v2/test/boot/algo-strict.sh` (quét cả cấu trúc, không chỉ hành vi).
#
# Dùng:
#   . v2/test/boot/build-identity.sh          # nạp hàm (tự nạp luôn build-rev.sh)
#   bash v2/test/boot/build-identity.sh --selftest
#
# Exit (khi chạy trực tiếp): 0 = selftest PASS; 1 = selftest FAIL; 2 = dùng sai.
set -u
LC_ALL=C
export LC_ALL

_BI_HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
. "$_BI_HERE/build-rev.sh"

# vbrev_image_identity <ảnh> -> 0 = ảnh tự khai ĐÚNG định nghĩa dấu vân tay hiện hành; != 0 = TỪ CHỐI.
#   Đặt VBREV_IMG_ALGO / VBREV_IMG_INPUTS / VBREV_IMG_DIRTY (rỗng nếu không đọc được)
#   và VBREV_IMG_REASON ∈ { ok, no-image, no-build-line, no-algo, algo-mismatch }.
#
# `no-build-line` và `no-algo` là **hai nguyên nhân khác nhau** và phải phân biệt: ảnh không có
# dòng danh tính nào (rác, hoặc ảnh không do kernel này build) khác với ảnh **có** dòng nhưng dạng
# cũ (thiếu `algo=`, tức build trước KT-143). Người đọc log cần biết mình đang cầm ảnh gì.
vbrev_image_identity() {
    local image="${1:-}" line="" rest=""
    VBREV_IMG_ALGO=""
    VBREV_IMG_INPUTS=""
    VBREV_IMG_DIRTY=""
    VBREV_IMG_REASON=""
    [ -f "$image" ] || {
        VBREV_IMG_REASON=no-image
        return 1
    }
    line="$(LC_ALL=C grep -a -o -m1 -E '# build: algo=[0-9A-Za-z._-]+ inputs=[0-9a-f]+ dirty=[01]' "$image" 2>/dev/null || true)"
    if [ -z "$line" ]; then
        if LC_ALL=C grep -a -q -E '# build: (inputs|rev)=[0-9a-f]+ dirty=[01]' "$image" 2>/dev/null; then
            VBREV_IMG_REASON=no-algo
        else
            VBREV_IMG_REASON=no-build-line
        fi
        return 1
    fi
    rest="${line#\# build: algo=}"
    VBREV_IMG_ALGO="${rest%% *}"
    rest="${rest#* inputs=}"
    VBREV_IMG_INPUTS="${rest%% *}"
    VBREV_IMG_DIRTY="${rest##* dirty=}"
    if [ "$VBREV_IMG_ALGO" != "$VBREV_ALGO" ]; then
        VBREV_IMG_REASON=algo-mismatch
        return 1
    fi
    VBREV_IMG_REASON=ok
    return 0
}

# ------------------------------------------------------------------ selftest
vbi_selftest() {
    local scratch rc=0 fp11 lbad
    scratch="$(mktemp -d "${TMPDIR:-/tmp}/kt145-identity-selftest.XXXXXX")" || {
        echo "# build-identity-selftest: FAIL (mktemp)" >&2
        return 1
    }
    # shellcheck disable=SC2064
    trap "rm -rf '$scratch'" RETURN 2>/dev/null || true
    # Dấu vân tay **thật** của cây hiện tại — không dùng giá trị giả để "cho qua".
    if ! vbrev_inputs_calc "$_BI_HERE/../../.." >/dev/null 2>&1; then
        echo "# build-identity-selftest: FAIL (không tính được dấu vân tay cây hiện tại: ${VBREV_CALC_ERROR:-?})"
        rm -rf "$scratch"
        return 1
    fi
    fp11="$VBREV_CALC_SHORT"
    lbad=0

    # (l1) ảnh **dạng cũ**: có `inputs=`/`dirty=` nhưng **không** `algo=` ⇒ no-algo
    printf 'ELF-gia\n# build: inputs=%s dirty=0\n' "$fp11" >"$scratch/old.bin"
    if vbrev_image_identity "$scratch/old.bin"; then
        echo "# build-identity-selftest (l1) FAIL: ảnh dạng CŨ (thiếu algo=) mà hàm nhận — reason=$VBREV_IMG_REASON"
        lbad=1
    elif [ "$VBREV_IMG_REASON" != "no-algo" ]; then
        echo "# build-identity-selftest (l1) FAIL: ảnh dạng cũ phải là reason=no-algo, nhận '$VBREV_IMG_REASON'"
        lbad=1
    fi

    # (l2) `algo=` khác định nghĩa hiện hành ⇒ algo-mismatch
    printf 'ELF-gia\n# build: algo=kt000-khong-co inputs=%s dirty=0\n' "$fp11" >"$scratch/badalgo.bin"
    if vbrev_image_identity "$scratch/badalgo.bin"; then
        echo "# build-identity-selftest (l2) FAIL: ảnh algo KHÁC định nghĩa mà hàm nhận — reason=$VBREV_IMG_REASON"
        lbad=1
    elif [ "$VBREV_IMG_REASON" != "algo-mismatch" ]; then
        echo "# build-identity-selftest (l2) FAIL: ảnh algo khác phải là reason=algo-mismatch, nhận '$VBREV_IMG_REASON'"
        lbad=1
    fi

    # (l3) ĐỐI CHỨNG DƯƠNG: `algo=` đúng ⇒ nhận, và đọc ra đúng inputs/dirty.
    #      Thiếu ca này thì một hàm luôn-từ-chối cũng qua được (l1)/(l2).
    printf 'ELF-gia\n# build: algo=%s inputs=%s dirty=0\n' "$VBREV_ALGO" "$fp11" >"$scratch/ok.bin"
    if ! vbrev_image_identity "$scratch/ok.bin"; then
        echo "# build-identity-selftest (l3) FAIL: ảnh algo ĐÚNG mà hàm từ chối — reason=$VBREV_IMG_REASON"
        lbad=1
    elif [ "$VBREV_IMG_ALGO" != "$VBREV_ALGO" ] || [ "$VBREV_IMG_INPUTS" != "$fp11" ] || [ "$VBREV_IMG_DIRTY" != "0" ]; then
        echo "# build-identity-selftest (l3) FAIL: đọc sai danh tính ảnh (algo=$VBREV_IMG_ALGO inputs=$VBREV_IMG_INPUTS dirty=$VBREV_IMG_DIRTY)"
        lbad=1
    fi

    # (l4) ảnh **không có** dòng danh tính nào ⇒ no-build-line (khác (l1): không phải ảnh cũ)
    printf 'ELF-gia khong co dong danh tinh\n' >"$scratch/noline.bin"
    if vbrev_image_identity "$scratch/noline.bin"; then
        echo "# build-identity-selftest (l4) FAIL: ảnh không có dòng danh tính mà hàm nhận — reason=$VBREV_IMG_REASON"
        lbad=1
    elif [ "$VBREV_IMG_REASON" != "no-build-line" ]; then
        echo "# build-identity-selftest (l4) FAIL: ảnh không dòng danh tính phải là reason=no-build-line, nhận '$VBREV_IMG_REASON'"
        lbad=1
    fi

    # (l5) không có tệp ⇒ no-image
    if vbrev_image_identity "$scratch/khong-ton-tai.bin"; then
        echo "# build-identity-selftest (l5) FAIL: tệp không tồn tại mà hàm nhận — reason=$VBREV_IMG_REASON"
        lbad=1
    elif [ "$VBREV_IMG_REASON" != "no-image" ]; then
        echo "# build-identity-selftest (l5) FAIL: tệp không tồn tại phải là reason=no-image, nhận '$VBREV_IMG_REASON'"
        lbad=1
    fi

    rm -rf "$scratch"
    [ "$lbad" -eq 0 ] || rc=1
    if [ "$rc" -eq 0 ]; then
        echo "# build-identity-selftest: PASS (l1/l2/l3/l4/l5)"
    else
        echo "# build-identity-selftest: FAIL"
    fi
    return "$rc"
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    case "${1:-}" in
        --selftest) vbi_selftest ;;
        *)
            echo "dùng: bash build-identity.sh --selftest   (hoặc source từ runner)" >&2
            exit 2
            ;;
    esac
fi
