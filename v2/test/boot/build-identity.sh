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

    # (l6) **KT-243 (B6, D2-197)** — thay đổi **CHƯA COMMIT** chỉ ở `v2/docs` + `v2/agent` (đúng
    #      cách harness **ghi log trong lúc đo**) KHÔNG được đổi dấu vân tay, KHÔNG được làm
    #      `dirty=1`, và KHÔNG được đổi quyết định của cổng. Đây là ca dương còn thiếu của B6:
    #      ca (a) trong `build-rev.sh --selftest` chỉ kiểm tệp ngoài build-inputs **đã commit**;
    #      ca (b) kiểm kernel **chưa commit** ⇒ `dirty=1`. Thiếu đúng vế "log harness chưa commit".
    #      Ca này ĐỎ nếu ai đó bỏ `v2/agent`/`v2/docs` khỏi phần TRỪ của build-inputs (đo: thêm
    #      `v2/agent` vào `VBREV_INPUT_ROOTS` ⇒ `dirty=1` ⇒ ĐỎ). Dùng **delta** (trước/sau) nên
    #      không phụ thuộc cây đang sạch hay đang có thay đổi khác ngoài phạm vi.
    #      Vì sao đặt ở **tệp này** chứ không ở `build-rev.sh`: `build-rev.sh` bị **ghim** bởi
    #      `check-algo-pin.sh` (sửa tệp ⇒ phải tăng `VBREV_ALGO` ⇒ mọi ảnh cũ thành "định nghĩa
    #      khác"), trong khi thuật toán băm **không đổi một byte**. Cùng lý do đã tách tệp này
    #      khỏi `build-rev.sh` ở KT-145 (xem đầu tệp).
    local l6root l6fp_before l6fp_after l6dirty_before l6dirty_after l6gate_before l6gate_after l6lines
    local l6f1 l6f2
    l6root="$_BI_HERE/../../.."
    l6f1="$l6root/v2/agent/JOURNAL/.kt243-l6.$$"
    l6f2="$l6root/v2/docs/.kt243-l6.$$"
    l6fp_before="$fp11"
    l6dirty_before="$(vbrev_dirty "$l6root")"
    printf '# build: algo=%s inputs=%s dirty=0\n' "$VBREV_ALGO" "$l6fp_before" >"$scratch/l6.log"
    printf 'ELF-gia\n# build: algo=%s inputs=%s dirty=0\n' "$VBREV_ALGO" "$l6fp_before" >"$scratch/l6.bin"
    l6gate_before="$(vbrev_gate "$scratch/l6.log" "$l6root" "$scratch/l6.bin" 2>&1)"
    mkdir -p "$l6root/v2/agent/JOURNAL" "$l6root/v2/docs" 2>/dev/null || true
    printf 'log harness gia lap (KT-243 l6)\n' >"$l6f1" 2>/dev/null || true
    printf 'tai lieu gia lap (KT-243 l6)\n' >"$l6f2" 2>/dev/null || true
    if ! vbrev_inputs_calc "$l6root" >/dev/null 2>&1; then
        echo "# build-identity-selftest (l6) FAIL: không tính được dấu vân tay sau khi thêm tệp v2/agent+v2/docs (${VBREV_CALC_ERROR:-?})"
        lbad=1
    else
        l6fp_after="$VBREV_CALC_SHORT"
        l6dirty_after="$(vbrev_dirty "$l6root")"
        if [ "$l6fp_before" != "$l6fp_after" ]; then
            echo "# build-identity-selftest (l6) FAIL: thêm tệp CHƯA commit ở v2/agent+v2/docs mà dấu vân tay ĐỔI ($l6fp_before -> $l6fp_after)"
            lbad=1
        fi
        if [ "$l6dirty_before" != "$l6dirty_after" ]; then
            echo "# build-identity-selftest (l6) FAIL: thêm tệp CHƯA commit ở v2/agent+v2/docs mà dirty ĐỔI ($l6dirty_before -> $l6dirty_after) — phần TRỪ của build-inputs hỏng"
            lbad=1
        fi
        # Vế **quyết định**: hai tệp vừa tạo **không** được xuất hiện trong danh sách tệp bẩn.
        # Vì sao cần vế này (không chỉ so delta): nếu cây đang có thay đổi khác trong phạm vi thì
        # `dirty` đã là 1 **trước** phép thử ⇒ delta = 0 ⇒ đột biến "bỏ phần TRỪ" sẽ **lọt**.
        # Đo được: thêm `v2/agent` vào `VBREV_INPUT_ROOTS` ⇒ hai tệp này hiện trong danh sách.
        l6lines="$(vbrev_dirty_lines "$l6root" 2>/dev/null || true)"
        case "$l6lines" in
            *".kt243-l6.$$"*)
                echo "# build-identity-selftest (l6) FAIL: tệp CHƯA commit dưới v2/agent/v2/docs bị tính là bẩn — phần TRỪ của build-inputs hỏng"
                lbad=1
                ;;
        esac
        l6gate_after="$(vbrev_gate "$scratch/l6.log" "$l6root" "$scratch/l6.bin" 2>&1)"
        if [ "$l6gate_before" != "$l6gate_after" ]; then
            echo "# build-identity-selftest (l6) FAIL: quyết định của cổng ĐỔI sau khi chỉ thêm tệp v2/agent+v2/docs — [trước: $l6gate_before] [sau: $l6gate_after]"
            lbad=1
        fi
    fi
    rm -f "$l6f1" "$l6f2" 2>/dev/null || true

    rm -rf "$scratch"
    [ "$lbad" -eq 0 ] || rc=1
    if [ "$rc" -eq 0 ]; then
        echo "# build-identity-selftest: PASS (l1/l2/l3/l4/l5/l6)"
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
