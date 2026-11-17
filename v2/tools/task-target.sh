#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# KT-84 (D2-122.6) — `CARGO_TARGET_DIR` **riêng cho từng task**: `<repo>/target-task/<mã>`.
#
# Vì sao: hai task build song song vào cùng `target/` sẽ giẫm lên nhau (ảnh của task này bị task kia
# ghi đè giữa lúc đo). Mỗi task một thư mục ⇒ không giẫm; xoá thư mục là dọn sạch task đó.
#
# **Không** dùng cho build canonical: `v2/tools/repro-build.sh` cấm `CARGO_TARGET_DIR` nằm trong cây
# nguồn (A1-175) và tự tạo thư mục tạm. Hai đường khác nhau, có chủ ý.
#
# Dùng:
#   export CARGO_TARGET_DIR="$(bash v2/tools/task-target.sh KT-84)"
#   bash v2/tools/task-target.sh --list        # liệt kê thư mục task đang có
#   bash v2/tools/task-target.sh --clean KT-84 # xoá thư mục của một task
#   bash v2/tools/task-target.sh --selftest    # tự kiểm 6 ca (dương + âm)
#
# Gốc thư mục đổi được bằng `TASK_TARGET_BASE` (mặc định `<repo>/target-task`) — `--selftest`
# dùng nó để không tạo gì trong repo thật. **Hành vi mặc định không đổi.**
#
# Exit: 0 = in đường dẫn (đã tạo); 2 = dùng sai / mã không hợp lệ.
set -u
LC_ALL=C
export LC_ALL

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
V2_ROOT="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$V2_ROOT/.." && pwd)"
BASE="${TASK_TARGET_BASE:-$ROOT/target-task}"

die() {
    echo "task-target: $*" >&2
    exit 2
}

valid_id() {
    case "${1:-}" in
        '' | *[!A-Za-z0-9._-]*) return 1 ;;
        *) return 0 ;;
    esac
}

# --- chế độ --selftest: dựng thư mục tạm, kiểm cả nhánh nhận và nhánh từ chối ------------------
# Mọi lần gọi con đều đặt `TASK_TARGET_BASE` vào thư mục tạm ⇒ **không** tạo gì trong repo thật.
if [ "${1:-}" = "--selftest" ]; then
    TMP="$(mktemp -d "${TMPDIR:-/tmp}/task-target-selftest.XXXXXX")" || die "mktemp"
    trap 'rm -rf "$TMP"' EXIT
    rc=0
    SB="$TMP/base" # BASE giả (thay cho <repo>/target-task)
    tt() { TASK_TARGET_BASE="$SB" bash "$0" "$@"; }

    # (a) mã hợp lệ ⇒ in đường dẫn, thư mục **được tạo**
    OUT="$(tt KT-98 2>&1)"
    st=$?
    if [ "$st" -ne 0 ]; then
        echo "# task-target-selftest (a) FAIL: mã hợp lệ trả mã $st (mong 0)"
        rc=1
    elif [ "$OUT" != "$SB/KT-98" ]; then
        echo "# task-target-selftest (a) FAIL: in '$OUT' (mong '$SB/KT-98')"
        rc=1
    elif [ ! -d "$SB/KT-98" ]; then
        echo "# task-target-selftest (a) FAIL: thư mục $SB/KT-98 không được tạo"
        rc=1
    fi
    # (b) --list thấy thư mục vừa tạo
    if ! tt --list 2>/dev/null | grep -qxF "$SB/KT-98"; then
        echo "# task-target-selftest (b) FAIL: --list không thấy $SB/KT-98"
        rc=1
    fi
    # (c) --clean <mã> xoá đúng thư mục đó, --list không còn
    tt --clean KT-98 >/dev/null 2>&1
    st=$?
    if [ "$st" -ne 0 ]; then
        echo "# task-target-selftest (c) FAIL: --clean trả mã $st (mong 0)"
        rc=1
    elif [ -d "$SB/KT-98" ]; then
        echo "# task-target-selftest (c) FAIL: thư mục vẫn còn sau --clean"
        rc=1
    elif tt --list 2>/dev/null | grep -qxF "$SB/KT-98"; then
        echo "# task-target-selftest (c) FAIL: --list vẫn thấy thư mục đã xoá"
        rc=1
    fi
    # (d) mã **không hợp lệ** (`../etc`, `a b`, rỗng) ⇒ mã thoát **hai**, và không tạo gì ngoài BASE
    for bad in '../etc' 'a b' ''; do
        tt "$bad" >/dev/null 2>&1
        st=$?
        [ "$st" -eq 2 ] || {
            echo "# task-target-selftest (d) FAIL: mã không hợp lệ '$bad' trả mã $st (mong 2)"
            rc=1
        }
    done
    tt --clean '../etc' >/dev/null 2>&1
    st=$?
    [ "$st" -eq 2 ] || {
        echo "# task-target-selftest (d) FAIL: --clean '../etc' trả mã $st (mong 2)"
        rc=1
    }
    [ ! -e "$TMP/etc" ] || {
        echo "# task-target-selftest (d) FAIL: '../etc' đã tạo $TMP/etc (thoát khỏi BASE)"
        rc=1
    }
    # (e) --clean mã không tồn tại ⇒ mã thoát **hai**
    tt --clean KT-404 >/dev/null 2>&1
    st=$?
    [ "$st" -eq 2 ] || {
        echo "# task-target-selftest (e) FAIL: --clean mã không tồn tại trả mã $st (mong 2)"
        rc=1
    }
    # (f) --clean **không** xoá gì ngoài thư mục của đúng task đó
    printf 'bait\n' >"$TMP/bait.txt" # tệp mồi BÊN CẠNH target-task/
    mkdir -p "$SB"
    printf 'keep\n' >"$SB/keepme.txt" # tệp lạ BÊN TRONG target-task/
    tt KT-97 >/dev/null 2>&1
    tt KT-96 >/dev/null 2>&1
    tt --clean KT-97 >/dev/null 2>&1
    [ -f "$TMP/bait.txt" ] || {
        echo "# task-target-selftest (f) FAIL: --clean xoá tệp mồi bên cạnh target-task/"
        rc=1
    }
    [ -f "$SB/keepme.txt" ] || {
        echo "# task-target-selftest (f) FAIL: --clean xoá tệp lạ trong target-task/"
        rc=1
    }
    [ -d "$SB/KT-96" ] || {
        echo "# task-target-selftest (f) FAIL: --clean xoá nhầm thư mục của task khác (KT-96)"
        rc=1
    }
    if [ "$rc" -eq 0 ]; then
        echo "# task-target-selftest: PASS (a/b/c/d/e/f)"
    else
        echo "# task-target-selftest: FAIL"
    fi
    exit "$rc"
fi

case "${1:-}" in
    --list)
        [ -d "$BASE" ] || exit 0
        find "$BASE" -mindepth 1 -maxdepth 1 -type d -print | LC_ALL=C sort
        exit 0
        ;;
    --clean)
        [ "$#" -eq 2 ] || die "--clean cần đúng một mã task"
        valid_id "$2" || die "mã task không hợp lệ: $2"
        DIR="$BASE/$2"
        [ -d "$DIR" ] || die "không có thư mục: $DIR"
        # Chỉ xoá đúng thư mục con của target-task, không xoá gì khác.
        case "$DIR" in
            "$BASE"/*) rm -rf -- "$DIR" ;;
            *) die "đường dẫn lạ: $DIR" ;;
        esac
        echo "task-target: đã xoá $DIR"
        exit 0
        ;;
esac

[ "$#" -eq 1 ] || die "dùng: task-target.sh <mã-task> | --list | --clean <mã>"
valid_id "$1" || die "mã task không hợp lệ: $1"
DIR="$BASE/$1"
mkdir -p "$DIR" || die "không tạo được $DIR"
echo "$DIR"
