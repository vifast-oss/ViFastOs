#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-134 (C1 của Agent 0) — **cổng ghim phiên bản thuật toán** của dấu vân tay build-inputs.
#
# Vì sao có cổng này (lỗ hổng mà KT-134 để lại):
#   KT-134 loại `v2/test/boot/build-rev.sh` khỏi danh sách ghim, vì nó là **định nghĩa** dấu vân
#   tay chứ không phải đầu vào của ảnh. Cái giá phải trả: sửa **thuật toán** trong script mà quên
#   tăng hằng số `VBREV_ALGO` thì dấu vân tay **không đổi**, nên dấu vân tay cũ và mới trở nên
#   **không so được** với nhau — và **không cổng nào** phát hiện. Cổng này bắt đúng ca đó bằng
#   cách so **cặp** (sha256 của script, `VBREV_ALGO`) với tệp ghim `v2/test/boot/build-rev.algo.pin`.
#
# Logic — đúng ba nhánh cho phần "khớp", không có nhánh thứ tư:
#   sha hiện tại == sha ghim                     ⇒ XANH.
#   sha khác, algo hiện tại == algo ghim         ⇒ ĐỎ `E_ALGO_NOT_BUMPED`
#                                                  (script đã đổi mà chưa tăng hằng số).
#   sha khác, algo hiện tại != algo ghim         ⇒ XANH + nhắc `W_PIN_STALE`
#                                                  (đã tăng hằng số đúng cách; cập nhật ghim).
#   tệp ghim thiếu / hỏng                        ⇒ ĐỎ `E_PIN_MISSING` / `E_PIN_MALFORMED`.
#
# **Vì sao "sha khác + algo khác" vẫn XANH mà chỉ nhắc**: đó là ca hợp lệ (ai đó đã tăng hằng số
# đúng cách). Nhưng nếu để yên, tệp ghim **cũ** sẽ khiến cổng **mãi mãi xanh** (lần sửa sau cũng
# "sha khác + algo khác" so với ghim cũ) ⇒ cổng mất tác dụng. Vì vậy có `--strict`: chế độ dùng
# cho cổng/CI, biến ghim cũ thành ĐỎ `E_PIN_STALE` để buộc cập nhật ghim **cùng commit** với lần
# tăng hằng số. Mặc định vẫn XANH đúng như đặc tả, để không chặn người đang làm dở.
#
# Dùng:
#   bash v2/tools/check-algo-pin.sh                # kiểm; 0 = xanh, 1 = đỏ
#   bash v2/tools/check-algo-pin.sh --strict       # như trên, nhưng ghim cũ (`W_PIN_STALE`) là ĐỎ
#   bash v2/tools/check-algo-pin.sh --update       # ghi lại ghim SAU KHI đã tăng VBREV_ALGO
#   bash v2/tools/check-algo-pin.sh --selftest     # 11 ca (dương + âm), dựng cây giả trong /tmp
#   ... --root DIR                                 # kiểm một cây khác (dùng cho --selftest)
#
# Exit: 0 = xanh; 1 = đỏ; 2 = dùng sai.
set -u
LC_ALL=C
export LC_ALL

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SELF="$HERE/$(basename "${BASH_SOURCE[0]}")"
V2_ROOT="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$V2_ROOT/.." && pwd)"
PIN="$V2_ROOT/test/boot/build-rev.algo.pin"
SCRIPT="$V2_ROOT/test/boot/build-rev.sh"

die() {
    echo "check-algo-pin: $*" >&2
    exit 2
}

usage() {
    cat >&2 <<'EOF'
dùng: bash v2/tools/check-algo-pin.sh [--strict] [--root DIR]
      bash v2/tools/check-algo-pin.sh --update [--root DIR]
      bash v2/tools/check-algo-pin.sh --selftest
EOF
}

# ---------------------------------------------------------------- công cụ
# sha256_file <tệp> -> 64 hex; != 0 nếu không có công cụ hoặc không đọc được.
# Cùng thứ tự ưu tiên với `vbrev_sha256_file` trong build-rev.sh (shasum trước, sha256sum sau)
# để hai bên không lệch nhau trên macOS/Linux.
sha256_file() {
    local f="${1:-}"
    [ -f "$f" ] || return 1
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$f" 2>/dev/null | awk '{print $1}'
    elif command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$f" 2>/dev/null | awk '{print $1}'
    else
        return 1
    fi
}

# read_algo <script> -> in giá trị VBREV_ALGO; != 0 nếu không đọc được **đúng một** giá trị.
# Vì sao đòi **đúng một**: nếu có hai dòng `VBREV_ALGO=` (ví dụ ai đó thêm biến cục bộ trùng tên),
# "giá trị hiện tại" là mơ hồ ⇒ fail-closed, không đoán. Dòng chú thích `# VBREV_ALGO=…` không
# khớp vì mẫu neo ở đầu dòng (chỉ cho phép khoảng trắng).
read_algo() {
    local f="${1:-}" n="" v=""
    [ -f "$f" ] || return 1
    n="$(grep -cE '^[[:space:]]*VBREV_ALGO=' "$f" 2>/dev/null || true)"
    [ -n "$n" ] || n=0
    [ "$n" = "1" ] || return 1
    v="$(sed -n 's/^[[:space:]]*VBREV_ALGO=//p' "$f" 2>/dev/null | head -1)"
    # Bỏ nháy bao ngoài (script hiện dùng nháy kép; chấp nhận cả nháy đơn và không nháy).
    case "$v" in
        \"*\") v="${v#\"}" && v="${v%\"}" ;;
        \'*\') v="${v#\'}" && v="${v%\'}" ;;
    esac
    # Giá trị hợp lệ: chữ/số/._- . Không khoảng trắng, không `$`, không nháy lẫn ⇒ không thể
    # "lách" bằng một biểu thức shell.
    case "$v" in
        '' | *[!A-Za-z0-9._-]*) return 1 ;;
    esac
    printf '%s' "$v"
}

# read_pin <pin> -> đặt PIN_ALGO / PIN_SHA.
#   0 = đọc được; 1 = tệp không có (E_PIN_MISSING); 2 = hỏng (E_PIN_MALFORMED).
# Định dạng **chặt** (fail-closed): chỉ chấp nhận dòng trống, dòng `#`, và **đúng một** dòng cho
# mỗi khoá. Dòng lạ ⇒ hỏng, không bỏ qua — bỏ qua là để lỗi gõ phím âm thầm vô hiệu cổng.
read_pin() {
    local f="${1:-}" line="" n_algo=0 n_sha=0 a="" s=""
    PIN_ALGO=""
    PIN_SHA=""
    [ -f "$f" ] || return 1
    [ -r "$f" ] || return 2
    while IFS= read -r line || [ -n "$line" ]; do
        case "$line" in
            '' | '#'*) continue ;;
            VBREV_ALGO_PIN=*)
                n_algo=$((n_algo + 1))
                a="${line#VBREV_ALGO_PIN=}"
                ;;
            VBREV_SCRIPT_SHA256=*)
                n_sha=$((n_sha + 1))
                s="${line#VBREV_SCRIPT_SHA256=}"
                ;;
            *) return 2 ;;
        esac
    done <"$f"
    [ "$n_algo" -eq 1 ] && [ "$n_sha" -eq 1 ] || return 2
    case "$a" in
        '' | *[!A-Za-z0-9._-]*) return 2 ;;
    esac
    case "$s" in
        '' | *[!0-9a-f]*) return 2 ;;
    esac
    [ "${#s}" -eq 64 ] || return 2
    PIN_ALGO="$a"
    PIN_SHA="$s"
    return 0
}

# write_pin <pin> <algo> <sha> — sinh lại **toàn bộ** tệp ghim từ mẫu trong script này.
# Vì sao sinh lại thay vì `sed -i` hai dòng giá trị: tệp ghim là **đầu ra xác định** của script,
# nên không có đường nào để hai bản (mẫu và tệp) lệch nhau. Ghi qua tệp tạm rồi `mv` để không bao
# giờ để lại tệp ghim nửa vời nếu bị ngắt giữa chừng.
write_pin() {
    local f="${1:-}" algo="${2:-}" sha="${3:-}" tmp=""
    [ -n "$f" ] && [ -n "$algo" ] && [ -n "$sha" ] || return 1
    tmp="$f.tmp.$$"
    {
        cat <<'HDR'
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-134 (C1 của Agent 0) — **GHIM PHIÊN BẢN THUẬT TOÁN** của dấu vân tay build-inputs.
#
# Vì sao tệp này tồn tại:
#   KT-134 loại `v2/test/boot/build-rev.sh` khỏi danh sách ghim (nó là **định nghĩa** dấu vân tay,
#   không phải đầu vào của ảnh). Hệ quả: sửa **thuật toán** trong script mà quên tăng `VBREV_ALGO`
#   thì dấu vân tay **không đổi** — dấu vân tay cũ và mới trở nên **không so được** với nhau, và
#   không cổng nào phát hiện. Tệp này ghim **cặp** (sha256 của script, giá trị `VBREV_ALGO`) để
#   `v2/tools/check-algo-pin.sh` phát hiện đúng ca đó.
#
# Định dạng: `KEY=value` trần, **không** dấu nháy, **không** khoảng trắng — đọc được bằng shell
# thuần (`sed -n 's/^KEY=//p'`) và cũng `source` được. Không dùng TOML/JSON để không cần python.
#   VBREV_ALGO_PIN       — giá trị `VBREV_ALGO` tại thời điểm ghim (hằng số này nằm trong chuỗi băm).
#   VBREV_SCRIPT_SHA256  — sha256 của `v2/test/boot/build-rev.sh` tại thời điểm ghim.
#
# Cách dùng:
#   bash v2/tools/check-algo-pin.sh            # kiểm (xanh/đỏ)
#   bash v2/tools/check-algo-pin.sh --update   # ghi lại ghim SAU KHI đã tăng VBREV_ALGO
#
# **Tệp này KHÔNG nằm trong build-inputs** (không thuộc `VBREV_INPUT_ROOTS`/`VBREV_INPUT_FILES`)
# và **không được** thêm vào: nó là công cụ kiểm, không phải đầu vào của ảnh. Thêm vào sẽ làm dấu
# vân tay phụ thuộc chính bản ghim của nó (vòng tròn), trái định nghĩa Q3 mà KT-134 đã chốt.
HDR
        printf 'VBREV_ALGO_PIN=%s\n' "$algo"
        printf 'VBREV_SCRIPT_SHA256=%s\n' "$sha"
    } >"$tmp" 2>/dev/null || {
        rm -f "$tmp"
        return 1
    }
    mv -f "$tmp" "$f" 2>/dev/null || {
        rm -f "$tmp"
        return 1
    }
    return 0
}

# ---------------------------------------------------------------- in kết quả
# Dòng máy grep được, cùng kiểu với `REV-GATE: REFUSE reason=…` của build-rev.sh.
red() { # <mã-lý-do> [<chi-tiết>]
    echo "ALGO-PIN: RED reason=$1${2:+ $2}"
}

ok_line() { # <sha> <algo> <state>
    echo "ALGO-PIN: OK sha=$1 algo=$2 pin_sha=$PIN_SHA pin_algo=$PIN_ALGO $3"
}

# warn_untracked — tệp ghim chưa được git theo dõi ⇒ cổng này **không đi theo repo** cho người
# khác (máy khác, CI) ⇒ cảnh báo, không đỏ: lúc đang tạo ghim thì chưa commit là bình thường.
warn_untracked() {
    local top=""
    command -v git >/dev/null 2>&1 || return 0
    top="$(git -C "$ROOT" --no-optional-locks rev-parse --show-toplevel 2>/dev/null)" || return 0
    [ -n "$top" ] || return 0
    git -C "$top" --no-optional-locks ls-files --error-unmatch -- "$PIN" >/dev/null 2>&1 && return 0
    echo "ALGO-PIN: WARN reason=W_PIN_UNTRACKED pin=$PIN"
    echo "check-algo-pin: tệp ghim CHƯA được git theo dõi — cổng này sẽ không đi theo repo cho người khác. Hãy commit nó." >&2
    return 0
}

# ---------------------------------------------------------------- kiểm
do_check() { # <strict 0|1>
    local strict="${1:-0}" sha_cur="" algo_cur="" rc_pin=0
    if [ ! -f "$SCRIPT" ]; then
        red E_SCRIPT_MISSING "script=$SCRIPT"
        return 1
    fi
    if ! algo_cur="$(read_algo "$SCRIPT")"; then
        red E_ALGO_UNREADABLE "script=$SCRIPT"
        echo "check-algo-pin: không đọc được **đúng một** dòng 'VBREV_ALGO=…' trong $SCRIPT." >&2
        return 1
    fi
    if ! sha_cur="$(sha256_file "$SCRIPT")" || [ "${#sha_cur}" -ne 64 ]; then
        red E_NO_SHA256 "script=$SCRIPT"
        echo "check-algo-pin: không tính được sha256 (cần shasum hoặc sha256sum)." >&2
        return 1
    fi
    read_pin "$PIN"
    rc_pin=$?
    case "$rc_pin" in
        0) ;;
        1)
            red E_PIN_MISSING "pin=$PIN"
            echo "check-algo-pin: thiếu tệp ghim ⇒ không có gì để so ⇒ fail-closed. Tạo bằng: bash v2/tools/check-algo-pin.sh --update" >&2
            return 1
            ;;
        *)
            red E_PIN_MALFORMED "pin=$PIN"
            echo "check-algo-pin: tệp ghim hỏng (thiếu khoá, khoá lặp, giá trị sai, hoặc có dòng lạ)." >&2
            echo "check-algo-pin: sửa tay hoặc xoá hẳn rồi chạy: bash v2/tools/check-algo-pin.sh --update" >&2
            return 1
            ;;
    esac

    # Nhánh 1: script **không đổi** so với lúc ghim.
    if [ "$sha_cur" = "$PIN_SHA" ]; then
        # Cùng nội dung script mà khác algo ⇒ tệp ghim bị sửa tay (bất khả về mặt logic).
        if [ "$algo_cur" != "$PIN_ALGO" ]; then
            red E_PIN_MALFORMED "pin=$PIN detail=algo-khong-khop-script pin_algo=$PIN_ALGO algo=$algo_cur"
            echo "check-algo-pin: script KHÔNG đổi nhưng algo trong ghim khác ⇒ tệp ghim bị sửa tay." >&2
            return 1
        fi
        warn_untracked
        ok_line "$sha_cur" "$algo_cur" "state=khop"
        return 0
    fi

    # Nhánh 2: script **đã đổi** mà hằng số **chưa** tăng ⇒ đúng lỗ hổng KT-134.
    if [ "$algo_cur" = "$PIN_ALGO" ]; then
        red E_ALGO_NOT_BUMPED "script=$SCRIPT sha=$sha_cur pin_sha=$PIN_SHA algo=$algo_cur"
        echo "check-algo-pin: build-rev.sh đã đổi nhưng VBREV_ALGO chưa tăng — dấu vân tay cũ và mới không so được." >&2
        echo "check-algo-pin: tăng VBREV_ALGO trong $SCRIPT rồi chạy: bash v2/tools/check-algo-pin.sh --update" >&2
        return 1
    fi

    # Nhánh 3: script đổi **và** hằng số đã tăng ⇒ hợp lệ, nhưng ghim còn cũ.
    if [ "$strict" = "1" ]; then
        red E_PIN_STALE "pin_algo=$PIN_ALGO algo=$algo_cur pin_sha=$PIN_SHA sha=$sha_cur"
        echo "check-algo-pin: VBREV_ALGO đã tăng ($PIN_ALGO -> $algo_cur) nhưng tệp ghim CHƯA cập nhật." >&2
        echo "check-algo-pin: chạy: bash v2/tools/check-algo-pin.sh --update  (rồi commit cùng lần tăng)." >&2
        return 1
    fi
    warn_untracked
    echo "ALGO-PIN: WARN reason=W_PIN_STALE pin_algo=$PIN_ALGO algo=$algo_cur pin_sha=$PIN_SHA sha=$sha_cur"
    echo "check-algo-pin: đã tăng VBREV_ALGO ($PIN_ALGO -> $algo_cur) — ĐÚNG cách. Còn thiếu: cập nhật tệp ghim bằng 'bash v2/tools/check-algo-pin.sh --update' (cùng commit với lần tăng)." >&2
    ok_line "$sha_cur" "$algo_cur" "state=algo-da-tang"
    return 0
}

# ---------------------------------------------------------------- ghi ghim
do_update() {
    local sha_cur="" algo_cur="" rc_pin=0
    [ -f "$SCRIPT" ] || {
        red E_SCRIPT_MISSING "script=$SCRIPT"
        return 1
    }
    if ! algo_cur="$(read_algo "$SCRIPT")"; then
        red E_ALGO_UNREADABLE "script=$SCRIPT"
        return 1
    fi
    if ! sha_cur="$(sha256_file "$SCRIPT")" || [ "${#sha_cur}" -ne 64 ]; then
        red E_NO_SHA256 "script=$SCRIPT"
        return 1
    fi
    read_pin "$PIN"
    rc_pin=$?
    case "$rc_pin" in
        0)
            if [ "$sha_cur" = "$PIN_SHA" ] && [ "$algo_cur" = "$PIN_ALGO" ]; then
                echo "ALGO-PIN: OK sha=$sha_cur algo=$algo_cur state=khong-doi pin=$PIN"
                return 0
            fi
            # **Chốt chặn quan trọng nhất của --update**: không được dùng --update để "rửa" một
            # thay đổi script chưa tăng hằng số. Nếu cho phép, toàn bộ cơ chế ghim vô nghĩa.
            if [ "$sha_cur" != "$PIN_SHA" ] && [ "$algo_cur" = "$PIN_ALGO" ]; then
                red E_UPDATE_REFUSED "detail=algo-not-bumped sha=$sha_cur pin_sha=$PIN_SHA algo=$algo_cur"
                echo "check-algo-pin: TỪ CHỐI ghi ghim: build-rev.sh đã đổi nhưng VBREV_ALGO chưa tăng." >&2
                echo "check-algo-pin: --update chỉ dùng SAU KHI tăng VBREV_ALGO; ghi đè ở đây sẽ xoá mất bằng chứng." >&2
                return 1
            fi
            ;;
        1)
            # Chưa có ghim ⇒ tạo mới. Không thể "rửa" gì vì chưa có mốc cũ để so.
            ;;
        *)
            red E_PIN_MALFORMED "pin=$PIN"
            echo "check-algo-pin: tệp ghim hỏng — không ghi đè lên trạng thái không đọc được (fail-closed)." >&2
            echo "check-algo-pin: sửa tay hoặc xoá hẳn rồi chạy lại --update." >&2
            return 1
            ;;
    esac
    write_pin "$PIN" "$algo_cur" "$sha_cur" || {
        red E_PIN_WRITE "pin=$PIN"
        return 1
    }
    echo "ALGO-PIN: OK sha=$sha_cur algo=$algo_cur state=da-cap-nhat pin=$PIN"
    return 0
}

# ---------------------------------------------------------------- selftest
# Dựng **cây giả** trong thư mục tạm (không đụng repo thật), rồi kiểm từng ca bằng **đúng mã lý
# do** — không chỉ "exit != 0". Vì sao phải kiểm mã: một cổng hỏng luôn trả 1 vẫn "qua" mọi ca âm
# nếu chỉ so mã thoát; và một cổng hỏng luôn trả 0 vẫn "qua" mọi ca dương. Kiểm cả hai vế + mã.
do_selftest() {
    local tmp="" repo="" fake="" pin="" rc=0 ncase=0
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/algo-pin-selftest.XXXXXX" 2>/dev/null)" || {
        echo "# algo-pin-selftest: mktemp FAIL"
        return 1
    }
    SELFTEST_TMP="$tmp"
    trap 'rm -rf "${SELFTEST_TMP:-}"' EXIT
    repo="$tmp/repo"
    mkdir -p "$repo/v2/test/boot" "$repo/v2/tools" || {
        echo "# algo-pin-selftest: dựng cây giả FAIL"
        rm -rf "$tmp"
        return 1
    }
    fake="$repo/v2/test/boot/build-rev.sh"
    pin="$repo/v2/test/boot/build-rev.algo.pin"

    # mk_fake <algo> [<dòng-chú-thích-thêm>] — ghi lại script giả từ đầu (sha xác định theo algo).
    mk_fake() {
        {
            printf '#!/usr/bin/env bash\n'
            printf '# script GIA cho selftest — khong phai build-rev.sh that\n'
            printf 'VBREV_ALGO="%s"\n' "$1"
            if [ -n "${2:-}" ]; then
                printf '# %s\n' "$2"
            fi
            printf 'VBREV_INPUTS_LEN=16\n'
        } >"$fake"
    }

    # expect_green <tên> <chuỗi-phải-có> <lệnh...>
    expect_green() {
        local name="$1" needle="$2" o="" r=0
        shift 2
        o="$("$@" 2>&1)" || r=$?
        ncase=$((ncase + 1))
        if [ "$r" -ne 0 ]; then
            echo "# algo-pin-selftest ($name) FAIL: mong XANH nhưng exit=$r — [$o]"
            rc=1
            return 0
        fi
        case "$o" in
            *"$needle"*) ;;
            *)
                echo "# algo-pin-selftest ($name) FAIL: thiếu [$needle] — [$o]"
                rc=1
                ;;
        esac
        case "$o" in
            *"ALGO-PIN: RED"*)
                echo "# algo-pin-selftest ($name) FAIL: xanh nhưng có dòng RED — [$o]"
                rc=1
                ;;
        esac
    }

    # expect_red <tên> <mã-lý-do> <lệnh...>
    expect_red() {
        local name="$1" code="$2" o="" r=0
        shift 2
        o="$("$@" 2>&1)" || r=$?
        ncase=$((ncase + 1))
        if [ "$r" -eq 0 ]; then
            echo "# algo-pin-selftest ($name) FAIL: mong ĐỎ ($code) nhưng exit=0 — [$o]"
            rc=1
            return 0
        fi
        case "$o" in
            *"reason=$code"*) ;;
            *)
                echo "# algo-pin-selftest ($name) FAIL: mong reason=$code, nhận — [$o]"
                rc=1
                ;;
        esac
    }

    # --- dựng mốc: script kt134-v1 + ghim sinh bằng --update (đường tạo ghim thật)
    mk_fake kt134-v1
    if ! bash "$SELF" --root "$repo" --update >/dev/null 2>&1; then
        echo "# algo-pin-selftest: không tạo được ghim mốc bằng --update"
        rm -rf "$tmp"
        return 1
    fi
    [ -f "$pin" ] || {
        echo "# algo-pin-selftest: --update không sinh ra tệp ghim"
        rm -rf "$tmp"
        return 1
    }

    # (a) DƯƠNG: sha khớp, algo khớp ⇒ XANH
    expect_green "a" "ALGO-PIN: OK" bash "$SELF" --root "$repo"

    # (b) ÂM: sha khác, algo KHÔNG đổi ⇒ ĐỎ E_ALGO_NOT_BUMPED
    mk_fake kt134-v1 "them mot dong chu thich vo hai"
    expect_red "b" "E_ALGO_NOT_BUMPED" bash "$SELF" --root "$repo"

    # (g) ÂM: --update phải TỪ CHỐI "rửa" đúng trạng thái của (b)
    expect_red "g" "E_UPDATE_REFUSED" bash "$SELF" --root "$repo" --update

    # (c) DƯƠNG: sha khác, algo ĐÃ đổi ⇒ XANH + nhắc W_PIN_STALE
    mk_fake kt134-v2
    expect_green "c" "W_PIN_STALE" bash "$SELF" --root "$repo"

    # (i) ÂM: --strict biến ghim cũ thành ĐỎ E_PIN_STALE (chế độ cho cổng/CI)
    expect_red "i" "E_PIN_STALE" bash "$SELF" --root "$repo" --strict

    # (h) DƯƠNG: --update sau khi tăng hằng số ⇒ ghim mới, rồi kiểm lại phải XANH **không** nhắc
    expect_green "h1" "state=da-cap-nhat" bash "$SELF" --root "$repo" --update
    expect_green "h2" "state=khop" bash "$SELF" --root "$repo"

    # (d) ÂM: tệp ghim thiếu ⇒ ĐỎ E_PIN_MISSING
    rm -f "$pin"
    expect_red "d" "E_PIN_MISSING" bash "$SELF" --root "$repo"

    # (e) ÂM: ghim hỏng — sha không phải 64 hex ⇒ ĐỎ E_PIN_MALFORMED
    {
        printf '# ghim hong\n'
        printf 'VBREV_ALGO_PIN=kt134-v2\n'
        printf 'VBREV_SCRIPT_SHA256=zzzz\n'
    } >"$pin"
    expect_red "e" "E_PIN_MALFORMED" bash "$SELF" --root "$repo"

    # (f) ÂM: ghim hỏng — thiếu khoá sha ⇒ ĐỎ E_PIN_MALFORMED
    printf 'VBREV_ALGO_PIN=kt134-v2\n' >"$pin"
    expect_red "f" "E_PIN_MALFORMED" bash "$SELF" --root "$repo"

    # (j) ÂM: thiếu script ⇒ ĐỎ E_SCRIPT_MISSING
    rm -f "$fake"
    expect_red "j" "E_SCRIPT_MISSING" bash "$SELF" --root "$repo"

    # (k) ÂM: script không có dòng VBREV_ALGO ⇒ ĐỎ E_ALGO_UNREADABLE
    printf '#!/usr/bin/env bash\n# khong co hang so\n' >"$fake"
    expect_red "k" "E_ALGO_UNREADABLE" bash "$SELF" --root "$repo"

    rm -rf "$tmp"
    if [ "$rc" -eq 0 ]; then
        echo "# algo-pin-selftest: PASS ($ncase ca: a/b/c/d/e/f/g/h1/h2/i/j/k)"
    else
        echo "# algo-pin-selftest: FAIL"
    fi
    return "$rc"
}

# ---------------------------------------------------------------- vào việc
MODE="check"
STRICT=0
ROOT_ARG=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --selftest) MODE="selftest"; shift ;;
        --update) MODE="update"; shift ;;
        --strict) STRICT=1; shift ;;
        --root)
            [ "$#" -ge 2 ] || die "--root cần tham số"
            ROOT_ARG="$2"
            shift 2
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *) die "tham số lạ: $1" ;;
    esac
done

if [ -n "$ROOT_ARG" ]; then
    ROOT="$(cd "$ROOT_ARG" 2>/dev/null && pwd)" || die "không thấy --root $ROOT_ARG"
    V2_ROOT="$ROOT/v2"
    PIN="$V2_ROOT/test/boot/build-rev.algo.pin"
    SCRIPT="$V2_ROOT/test/boot/build-rev.sh"
fi

case "$MODE" in
    selftest) do_selftest ;;
    update) do_update ;;
    check) do_check "$STRICT" ;;
    *) die "chế độ lạ: $MODE" ;;
esac
